use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Path, RawQuery, State};
use axum::routing::delete;
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::db::Pool;
use crate::feature::git::{RunGitError, run_git_command};
use crate::feature::repo::{CreateRepoError, create_bare_repo};
use crate::feature::user;

#[derive(Debug, Clone)]
struct AppState {
    base_dir: PathBuf,
    pool: Arc<Pool>,
}

#[derive(Debug)]
struct RunGitQuery {
    path: Option<String>,
    arg: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunGitResponse {
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    cwd: String,
    command: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErrorResponse {
    error: String,
}

pub async fn serve(base_dir: PathBuf, pool: Arc<Pool>) -> Result<()> {
    let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_string());

    let app = build_router(base_dir, pool);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .with_context(|| format!("failed to bind HTTP server to {bind_addr}"))?;

    println!("HTTP listening on {bind_addr}");
    axum::serve(listener, app).await.context("HTTP server failed")?;
    Ok(())
}

pub fn build_router(base_dir: PathBuf, pool: Arc<Pool>) -> Router {
    Router::new()
        .route("/git/{operation}", get(run_git_handler))
        .route("/repos", post(create_repo_handler))
        // ユーザー管理
        .route("/users", get(list_users_handler).post(create_user_handler))
        .route("/users/{username}", get(get_user_handler))
        // SSH 鍵管理
        .route(
            "/users/{username}/keys",
            get(list_keys_handler).post(add_key_handler),
        )
        .route("/users/{username}/keys/{key_id}", delete(delete_key_handler))
        .layer(build_cors_layer())
        .with_state(AppState { base_dir, pool })
}

#[derive(Debug, Deserialize)]
struct CreateRepoRequest {
    name: String,
}

#[derive(Debug, Serialize)]
struct CreateRepoResponse {
    name: String,
}

async fn create_repo_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateRepoRequest>,
) -> Response {
    match create_bare_repo(&state.base_dir, &body.name).await {
        Ok(path) => {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&body.name)
                .to_string();
            (StatusCode::CREATED, Json(CreateRepoResponse { name })).into_response()
        }
        Err(CreateRepoError::InvalidName(msg)) => bad_request(msg),
        Err(CreateRepoError::AlreadyExists(name)) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("repository `{name}` already exists"),
            }),
        )
            .into_response(),
        Err(CreateRepoError::ExecutionFailed(msg)) => internal_error(msg),
    }
}

// ---------------------------------------------------------------------------
// ユーザー管理ハンドラー
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    username: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: i64,
    username: String,
    created_at: String,
}

impl From<user::User> for UserResponse {
    fn from(u: user::User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            created_at: u.created_at,
        }
    }
}

async fn list_users_handler(State(state): State<AppState>) -> Response {
    match user::list_users(&state.pool).await {
        Ok(users) => {
            let body: Vec<UserResponse> = users.into_iter().map(Into::into).collect();
            Json(body).into_response()
        }
        Err(e) => internal_error(e.to_string()),
    }
}

async fn create_user_handler(
    State(state): State<AppState>,
    Json(body): Json<CreateUserRequest>,
) -> Response {
    match user::create_user(&state.pool, &body.username).await {
        Ok(u) => (StatusCode::CREATED, Json(UserResponse::from(u))).into_response(),
        Err(e) => bad_request(e.to_string()),
    }
}

async fn get_user_handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Response {
    match user::find_user(&state.pool, &username).await {
        Ok(Some(u)) => Json(UserResponse::from(u)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("user `{username}` not found"),
            }),
        )
            .into_response(),
        Err(e) => internal_error(e.to_string()),
    }
}

// ---------------------------------------------------------------------------
// SSH 鍵管理ハンドラー
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AddKeyRequest {
    /// authorized_keys 形式の1行: `ssh-ed25519 AAAA... comment`
    key: String,
}

#[derive(Debug, Serialize)]
struct SshKeyResponse {
    id: i64,
    fingerprint: String,
    key_type: String,
    key_data: String,
    comment: String,
    created_at: String,
}

impl From<user::SshKey> for SshKeyResponse {
    fn from(k: user::SshKey) -> Self {
        Self {
            id: k.id,
            fingerprint: k.fingerprint,
            key_type: k.key_type,
            key_data: k.key_data,
            comment: k.comment,
            created_at: k.created_at,
        }
    }
}

async fn list_keys_handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Response {
    match user::list_ssh_keys(&state.pool, &username).await {
        Ok(keys) => {
            let body: Vec<SshKeyResponse> = keys.into_iter().map(Into::into).collect();
            Json(body).into_response()
        }
        Err(e) => internal_error(e.to_string()),
    }
}

async fn add_key_handler(
    State(state): State<AppState>,
    Path(username): Path<String>,
    Json(body): Json<AddKeyRequest>,
) -> Response {
    match user::add_ssh_key(&state.pool, &username, &body.key).await {
        Ok(key) => (StatusCode::CREATED, Json(SshKeyResponse::from(key))).into_response(),
        Err(e) => bad_request(e.to_string()),
    }
}

async fn delete_key_handler(
    State(state): State<AppState>,
    Path((username, key_id)): Path<(String, i64)>,
) -> Response {
    match user::delete_ssh_key(&state.pool, &username, key_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("key {key_id} not found for user `{username}`"),
            }),
        )
            .into_response(),
        Err(e) => internal_error(e.to_string()),
    }
}

fn build_cors_layer() -> CorsLayer {
    let allowed_origins_raw = env::var("CORS_ALLOW_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:3000,http://127.0.0.1:3000".to_string());

    let allowed_origins: Vec<axum::http::HeaderValue> = allowed_origins_raw
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| axum::http::HeaderValue::from_str(origin).ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    if allowed_origins.is_empty() {
        cors.allow_origin(Any)
    } else {
        cors.allow_origin(allowed_origins)
    }
}

async fn run_git_handler(
    State(state): State<AppState>,
    Path(operation): Path<String>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let query = parse_run_git_query(raw_query.as_deref());
    let path = match query.path {
        Some(path) if !path.trim().is_empty() => path,
        _ => {
            return bad_request("query parameter `path` is required".to_string());
        }
    };

    match run_git_command(&state.base_dir, &path, &operation, &query.arg).await {
        Ok(output) => (
            StatusCode::OK,
            Json(RunGitResponse {
                success: output.success,
                exit_code: output.exit_code,
                stdout: output.stdout,
                stderr: output.stderr,
                cwd: output.cwd,
                command: output.command,
            }),
        )
            .into_response(),
        Err(RunGitError::InvalidInput(message)) => bad_request(message),
        Err(RunGitError::ExecutionFailed(message)) => internal_error(message),
    }
}

fn parse_run_git_query(raw_query: Option<&str>) -> RunGitQuery {
    let mut path = None;
    let mut arg = Vec::new();

    if let Some(raw_query) = raw_query {
        for (key, value) in url::form_urlencoded::parse(raw_query.as_bytes()) {
            match key.as_ref() {
                "path" => path = Some(value.into_owned()),
                "arg" | "arg[]" => arg.push(value.into_owned()),
                _ => {}
            }
        }
    }

    RunGitQuery { path, arg }
}

fn bad_request(message: String) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse { error: message }),
    )
        .into_response()
}

fn internal_error(message: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{ErrorResponse, RunGitResponse, build_router};
    use axum::body::{Body, to_bytes};
    use axum::http::{Request, StatusCode, header};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    /// テスト用のインメモリ DB プールを作成する。
    async fn test_pool() -> Arc<crate::db::Pool> {
        Arc::new(
            crate::db::connect(":memory:")
                .await
                .expect("test DB should open"),
        )
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();

        path.push(format!(
            "api_server_http_{name}_{}_{}",
            std::process::id(),
            stamp
        ));
        path
    }

    fn init_repo(path: &Path) {
        fs::create_dir_all(path).expect("failed to create repository directory");
        let output = Command::new("git")
            .arg("init")
            .current_dir(path)
            .output()
            .expect("failed to run git init");

        assert!(
            output.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[tokio::test]
    async fn returns_200_with_success_true_for_valid_repo() {
        let base = unique_temp_path("valid");
        let repo = base.join("repo");
        init_repo(&repo);

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=repo")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: RunGitResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(payload.success);
        assert_eq!(payload.exit_code, Some(0));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_200_for_checkout_with_args() {
        let base = unique_temp_path("checkout_with_args");
        let repo = base.join("repo");
        init_repo(&repo);

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/checkout?path=repo&arg=feature")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: RunGitResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert_eq!(payload.command, vec!["checkout", "feature"]);

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_400_when_path_is_missing() {
        let base = unique_temp_path("missing_path");
        fs::create_dir_all(&base).expect("failed to create base dir");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: ErrorResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(payload.error.contains("path"));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_404_when_operation_is_missing() {
        let base = unique_temp_path("missing_cmd");
        fs::create_dir_all(&base).expect("failed to create base dir");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git?path=repo")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_400_for_unsupported_cmd() {
        let base = unique_temp_path("unsupported_cmd");
        let repo = base.join("repo");
        init_repo(&repo);

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/status?path=repo")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: ErrorResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(payload.error.contains("unsupported `cmd`"));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_400_for_path_escape_attempt() {
        let base = unique_temp_path("escape");
        fs::create_dir_all(base.join("repo")).expect("failed to create repo dir");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=../outside")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_400_for_absolute_path() {
        let base = unique_temp_path("absolute_path");
        fs::create_dir_all(base.join("repo")).expect("failed to create repo dir");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=/tmp")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: ErrorResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(payload.error.contains("path"));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn returns_400_for_symlink_escape_attempt() {
        use std::os::unix::fs::symlink;

        let base = unique_temp_path("symlink_escape_base");
        let outside = unique_temp_path("symlink_escape_outside");

        fs::create_dir_all(&base).expect("failed to create base dir");
        fs::create_dir_all(&outside).expect("failed to create outside dir");
        symlink(&outside, base.join("linked")).expect("failed to create symlink");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=linked")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        fs::remove_dir_all(base).expect("cleanup should succeed");
        fs::remove_dir_all(outside).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_200_with_success_false_for_non_repo() {
        let base = unique_temp_path("non_repo");
        fs::create_dir_all(base.join("dir")).expect("failed to create target dir");

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=dir")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: RunGitResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(!payload.success);
        assert_ne!(payload.exit_code, Some(0));

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }

    #[tokio::test]
    async fn returns_500_when_base_dir_is_not_accessible() {
        let base = unique_temp_path("missing_base_dir");

        let app = build_router(base, test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=repo")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: ErrorResponse =
            serde_json::from_slice(&body).expect("response should be valid json");

        assert!(payload.error.contains("base directory"));
    }

    #[tokio::test]
    async fn adds_access_control_allow_origin_for_allowed_origin() {
        let base = unique_temp_path("cors_allowed_origin");
        let repo = base.join("repo");
        init_repo(&repo);

        let app = build_router(base.clone(), test_pool().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/git/branch?path=repo")
                    .header(header::ORIGIN, "http://localhost:3000")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("router should respond");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .expect("cors header should exist"),
            "http://localhost:3000"
        );

        fs::remove_dir_all(base).expect("cleanup should succeed");
    }
}
