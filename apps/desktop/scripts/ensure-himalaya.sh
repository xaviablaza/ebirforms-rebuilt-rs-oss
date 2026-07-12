#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TAURI_DIR="$(cd "$SCRIPT_DIR/../src-tauri" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TOOLS_DIR="$REPO_ROOT/target/desktop-tools/himalaya"
BIN_DIR="$TAURI_DIR/binaries"
RESOURCES_DIR="$TAURI_DIR/resources"

host_triple="$(rustc -vV | sed -n 's/^host: //p')"
case "$(uname -s | tr '[:upper:]' '[:lower:]')" in
  msys*|mingw*|cygwin*|win*) binary="himalaya.exe" ;;
  *) binary="himalaya" ;;
esac

mkdir -p "$TOOLS_DIR" "$BIN_DIR" "$RESOURCES_DIR"

if [ ! -x "$TOOLS_DIR/bin/$binary" ]; then
  echo "Installing packaged Himalaya CLI into $TOOLS_DIR"
  curl -sSL https://raw.githubusercontent.com/pimalaya/himalaya/master/install.sh | PREFIX="$TOOLS_DIR" sh
fi

cp -f "$TOOLS_DIR/bin/$binary" "$RESOURCES_DIR/$binary"
if [ "$binary" = "himalaya.exe" ]; then
  cp -f "$TOOLS_DIR/bin/$binary" "$BIN_DIR/himalaya-$host_triple.exe"
else
  cp -f "$TOOLS_DIR/bin/$binary" "$BIN_DIR/himalaya-$host_triple"
fi
chmod +x "$RESOURCES_DIR/$binary" "$BIN_DIR"/himalaya-*

echo "Packaged Himalaya: $($RESOURCES_DIR/$binary --version)"
