CREATE TABLE users (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    username   TEXT    NOT NULL UNIQUE,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TABLE ssh_keys (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- SHA256:... 形式のフィンガープリント。SSH認証時の高速照合に使う。
    fingerprint TEXT    NOT NULL UNIQUE,
    key_type    TEXT    NOT NULL,
    -- authorized_keys 形式のbase64公開鍵データ（表示・復元用）
    key_data    TEXT    NOT NULL,
    comment     TEXT    NOT NULL DEFAULT '',
    created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
