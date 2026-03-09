#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="$SCRIPT_DIR/api-server/.env"

# リポジトリ保存ディレクトリを作成
mkdir -p "$SCRIPT_DIR/repositories"

# .env が既に存在する場合はスキップ
if [ -f "$ENV_FILE" ]; then
  echo "api-server/.env already exists, skipping."
  exit 0
fi

cat > "$ENV_FILE" <<EOF
GIT_BASE_DIR=$(realpath "$SCRIPT_DIR/repositories")
BIND_ADDR=127.0.0.1:8080
CORS_ALLOW_ORIGINS=http://localhost:3000,http://127.0.0.1:3000
EOF

echo "Created api-server/.env"
