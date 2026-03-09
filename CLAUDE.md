# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

**gitmarch** は Raspberry Pi 等で動作する自家製 Git ホスティングサービスです。開発者はここに `git push` し、Web UI でブランチ確認・Merge Request・CI などを操作できます（Gitea や GitLab の自作版）。

- `api-server/` — Rust (Axum) 製の HTTP API サーバー
- `web-ui/` — Next.js (App Router) + React 19 製のフロントエンド
- `repositories/` — ホストしている Git リポジトリの実体が置かれるディレクトリ

## api-server (Rust)

### コマンド

```bash
cd api-server

cargo build
cargo test

# 特定のテストのみ
cargo test <test_name>

# 開発実行（GIT_BASE_DIR は必須）
GIT_BASE_DIR=../repositories cargo run
```

### 環境変数

| 変数名 | 必須 | デフォルト | 説明 |
|---|---|---|---|
| `GIT_BASE_DIR` | 必須 | — | リポジトリが置かれたベースディレクトリ |
| `BIND_ADDR` | 任意 | `127.0.0.1:3000` | サーバーのバインドアドレス |
| `CORS_ALLOW_ORIGINS` | 任意 | `http://localhost:3000,http://127.0.0.1:3000` | カンマ区切りの許可オリジン |

### アーキテクチャ

- `src/main.rs` — エントリーポイント
- `src/feature/git.rs` — コアロジック。`run_git_command()` がパス検証とコマンド実行を担う
- `src/feature/git/{branch,checkout,merge,pull,push,switch}.rs` — 各 Git コマンドの実装
- `src/presentation/http.rs` — Axum ルーター

**APIエンドポイント**: `GET /git/{operation}?path=<repo>&arg[]=<arg1>`

許可されている operation: `branch`, `checkout`, `merge`, `pull`, `push`, `switch`

**セキュリティ**: `path` パラメーターは `GIT_BASE_DIR` 配下への相対パスのみ許可。絶対パス・`..`・シンボリックリンクによる脱出はすべて 400 で拒否する。

### コンテナビルド

```bash
cd api-server
podman build -f ContainerFile -t api-server .
podman run -e GIT_BASE_DIR=/repos -e BIND_ADDR=0.0.0.0:8080 -p 8080:8080 api-server
```

### 新しい Git 操作の追加パターン

1. `src/feature/git/<operation>.rs` に `execute()` を実装
2. `src/feature/git.rs` の `ALLOWED_COMMANDS` と `execute_operation()` に登録

## web-ui (Next.js)

### コマンド

```bash
cd web-ui

npm run dev       # 開発サーバー（:3000）
npm run build
npm test          # Vitest
npm run lint      # oxlint
npm run lint:fix
npm run format    # oxfmt
npm run format:check
```

### 環境変数

| 変数名 | デフォルト | 説明 |
|---|---|---|
| `NEXT_PUBLIC_API_SERVER_URL` または `API_SERVER_URL` | `http://127.0.0.1:8080` | api-server の URL |

### アーキテクチャ

```
src/
  app/                  # Next.js App Router（ルーティングのみ）
  presentation/pages/   # ページコンポーネント
  feature/git/          # Git 操作のロジックと UI コンポーネント
  infrastructure/       # API クライアント (client.ts)
  shared/               # 汎用 UI コンポーネント (shadcn/ui)、ユーティリティ
```

**パスエイリアス** (`@/*`, `feature/*`, `presentation/*`, `infrastructure/*`, `shared/*` → 各 `src/` 配下)
