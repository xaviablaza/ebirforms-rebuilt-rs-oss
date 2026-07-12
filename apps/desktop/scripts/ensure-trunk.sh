#!/usr/bin/env bash
set -euo pipefail

TRUNK_VERSION="0.21.14"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TOOL_ROOT="$ROOT/target/desktop-tools/trunk-$TRUNK_VERSION"
TRUNK_BIN="$TOOL_ROOT/bin/trunk"

rustup target add wasm32-unknown-unknown >/dev/null

if [[ ! -x "$TRUNK_BIN" ]]; then
  mise x rust@1.88.0 -- cargo install trunk --version "$TRUNK_VERSION" --locked --root "$TOOL_ROOT"
fi

cd "$ROOT/apps/desktop/frontend"
if [[ "${1:-}" == "build" ]]; then
  exec "$TRUNK_BIN" build --public-url ./ "${@:2}"
fi
exec "$TRUNK_BIN" "$@"
