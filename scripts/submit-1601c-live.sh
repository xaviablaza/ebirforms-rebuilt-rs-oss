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

input_path="${1:-}"
if [[ -z "$input_path" ]]; then
  echo "Usage: $0 /private/path/authorized-1601C-input.json" >&2
  exit 1
fi

exec cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input "$input_path" \
  --live --confirm \
  --records .ebirforms/submissions.json
