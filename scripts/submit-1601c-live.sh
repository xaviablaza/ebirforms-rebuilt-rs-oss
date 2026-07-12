#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_dir"

if [[ ! -f .env.local ]]; then
  echo "Missing .env.local with BIR_SFTP_* credentials" >&2
  exit 1
fi

set -a
# shellcheck disable=SC1091
source .env.local
set +a

exec cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --live --confirm \
  --records .ebirforms/submissions.json
