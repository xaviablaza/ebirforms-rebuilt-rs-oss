#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS_DIR="$REPO_ROOT/target/cli-tools/himalaya"
RELEASE_DIR="$REPO_ROOT/target/release"

case "$(uname -s | tr '[:upper:]' '[:lower:]')" in
  msys*|mingw*|cygwin*|win*) binary="himalaya.exe" ;;
  *) binary="himalaya" ;;
esac

mkdir -p "$TOOLS_DIR" "$RELEASE_DIR"
if [ ! -x "$TOOLS_DIR/bin/$binary" ]; then
  echo "Installing packaged Himalaya CLI into $TOOLS_DIR"
  curl -sSL https://raw.githubusercontent.com/pimalaya/himalaya/master/install.sh | PREFIX="$TOOLS_DIR" sh
fi
cp -f "$TOOLS_DIR/bin/$binary" "$RELEASE_DIR/$binary"
chmod +x "$RELEASE_DIR/$binary"
echo "Packaged CLI Himalaya sidecar: $($RELEASE_DIR/$binary --version)"
