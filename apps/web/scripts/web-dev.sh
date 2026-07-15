#!/usr/bin/env bash
set -euo pipefail

TRUNK_VERSION="0.21.14"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TOOL_ROOT="$ROOT/target/web-tools/trunk-$TRUNK_VERSION"
TRUNK_BIN="$TOOL_ROOT/bin/trunk"
STATE_DIR="$ROOT/.devenv/state"
KEY_FILE="$STATE_DIR/web-intake.key"

rustup target add wasm32-unknown-unknown >/dev/null

if [[ ! -x "$TRUNK_BIN" ]]; then
  cargo install trunk --version "$TRUNK_VERSION" --locked --root "$TOOL_ROOT"
fi

export EBIRFORMS_WEB_INSECURE_COOKIE=1
export EBIRFORMS_WEB_DB="${EBIRFORMS_WEB_DB:-$STATE_DIR/web-intake.sqlite3}"
export EBIRFORMS_WEB_FRONTEND_PORT="${EBIRFORMS_WEB_FRONTEND_PORT:-1421}"
export EBIRFORMS_WEB_API_PORT="${EBIRFORMS_WEB_API_PORT:-3001}"
export EBIRFORMS_WEB_BIND="127.0.0.1:$EBIRFORMS_WEB_API_PORT"

mkdir -p "$STATE_DIR"
if [[ ! -f "$KEY_FILE" ]]; then
  umask 077
  openssl rand -base64 32 > "$KEY_FILE"
fi
chmod 600 "$KEY_FILE"
export EBIRFORMS_WEB_ENCRYPTION_KEY="$(<"$KEY_FILE")"

cd "$ROOT"
cargo run -p ebirforms-web &
api_pid=$!
trap 'kill "$api_pid" 2>/dev/null || true' EXIT INT TERM

cd apps/web/frontend
NO_COLOR=false "$TRUNK_BIN" serve \
  --address 127.0.0.1 \
  --port "$EBIRFORMS_WEB_FRONTEND_PORT" \
  --proxy-backend="http://127.0.0.1:$EBIRFORMS_WEB_API_PORT/api/"
