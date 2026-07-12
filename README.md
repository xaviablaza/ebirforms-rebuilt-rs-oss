# eBIRForms Rebuilt Rust Workspace

Local Rust implementation for reproducing the eBIRForms submission payload pipeline discovered from the Windows app investigation.

## Current scope

Implemented programmatic-submission MVP slices from `docs/architecture/optimized-programmatic-submission-plan.md`:

- `ebirforms-core::crypto::{encrypt_payload,decrypt_payload}` for the confirmed zlib + DCPcrypt-compatible AES-256 transform.
- `ebirforms-core::form` for template-first 1601C rendering from JSON using bundled `form.toml`, `mapping.toml`, and `template.xml`.
- `ebirforms-core::package` for JSON → plaintext → encrypted upload artifact plus manifest, hashes, remote path, and filename.
- `ebirforms-core::transport` for safe dry-run submission receipts, idempotency-key duplicate protection, and a gated live SFTP abstraction.
- `ebirforms-core::submission` for durable JSON submission records, audit status, pre-network idempotency persistence, and `Uncertain` duplicate-risk blocking.
- `ebirforms-core::job` for a SQLite submission job queue, queued/running/final statuses, retry/backoff policy, and worker execution through the proven submit path.
- `ebirforms-core::profile` for desktop-ready taxpayer profiles, theme settings, and a basic local master-PIN verifier.
- `ebirforms-core::receipt` for fixture-proven receipt parsing/matching and local directory polling that confirms stored submissions without resubmitting.
- `ebirforms-cli` commands: `encrypt`, `decrypt`, `render`, `package`, `diff-fixture`, safe-by-default `submit`, queue commands (`queue`, `run-queue`, `jobs`), local IPC server (`serve`), profile/settings commands, and receipt commands (`receipt-match`, `receipt-poll`).
- Public redacted 1601C smoke fixtures under `tests/fixtures/1601C/` plus private captured fixture tests.

## Knowledge handoff

See:

```text
docs/session-knowledge-handoff.md
docs/architecture/optimized-programmatic-submission-plan.md
```

## Private fixtures

Captured taxpayer fixtures are intentionally gitignored:

```text
fixtures/private/1601c/plaintext-v2.xml
fixtures/private/1601c/encrypted-v2.xml
```

They are present locally on this machine so the fixture tests can verify exact byte compatibility. Do not commit real taxpayer data to a public repo.

Expected private fixture hashes:

```text
plaintext-v2.xml sha256: c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713
encrypted-v2.xml sha256: 8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f
```

## Commands

```bash
cargo test

# Existing low-level transform helpers
cargo run -p ebirforms-cli -- encrypt fixtures/private/1601c/plaintext-v2.xml /tmp/encrypted.xml
cargo run -p ebirforms-cli -- decrypt fixtures/private/1601c/encrypted-v2.xml /tmp/plaintext.xml

# Template-first MVP flow
cargo run -p ebirforms-cli -- render --form 1601C --input tests/fixtures/1601C/input.json --out /tmp/plaintext.xml
cargo run -p ebirforms-cli -- package --form 1601C --input tests/fixtures/1601C/input.json --out /tmp/upload.xml --manifest /tmp/manifest.json
cargo run -p ebirforms-cli -- diff-fixture --form 1601C --input tests/fixtures/1601C/input.json --fixture tests/fixtures/1601C/official_encrypted.xml
cargo run -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --dry-run --records /tmp/ebirforms-submissions.json
cargo run -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --live --confirm --records /tmp/ebirforms-live-submissions.json

# Queue/worker MVP flow
cargo run -p ebirforms-cli -- queue --form 1601C --input tests/fixtures/1601C/input.json --dry-run --db /tmp/ebirforms-jobs.sqlite
cargo run -p ebirforms-cli -- run-queue --dry-run --db /tmp/ebirforms-jobs.sqlite --records /tmp/ebirforms-submissions.json --limit 1
cargo run -p ebirforms-cli -- jobs --db /tmp/ebirforms-jobs.sqlite

# Local daemon/IPC slice
cargo run -p ebirforms-cli -- serve --addr 127.0.0.1:8765 --db /tmp/ebirforms-jobs.sqlite --records /tmp/ebirforms-submissions.json
curl http://127.0.0.1:8765/health
curl -X POST 'http://127.0.0.1:8765/jobs?form=1601C&mode=dry_run' --data-binary @tests/fixtures/1601C/input.json
curl -X POST 'http://127.0.0.1:8765/run-queue?mode=dry_run&limit=1'

# Desktop-ready profile/settings slice
cargo run -p ebirforms-cli -- profile-create --profile-id redacted-test-profile --tin 123-456-789-00000 --email authorized@example.test --name 'AUTHORIZED TEST TAXPAYER' --rdo 044 --state /tmp/ebirforms-app-state.json
cargo run -p ebirforms-cli -- settings --theme dark --state /tmp/ebirforms-app-state.json
cargo run -p ebirforms-cli -- lock-init --pin 1234 --state /tmp/ebirforms-app-state.json
cargo run -p ebirforms-cli -- unlock-check --pin 1234 --state /tmp/ebirforms-app-state.json

# Receipt matching / polling fixtures
cargo run -p ebirforms-cli -- receipt-match --receipt tests/fixtures/1601C/receipt_accepted.txt --records /tmp/ebirforms-submissions.json
cargo run -p ebirforms-cli -- receipt-poll --receipt-dir /tmp/ebirforms-receipts --records /tmp/ebirforms-submissions.json
```

The default persistent stores are `.ebirforms/submissions.json` for latest-state submission audit records and `.ebirforms/jobs.sqlite` for the queue; `.ebirforms/` is gitignored. Use `--records <path>` and `--db <path>` for test runs.

Live submission is gated behind `--live --confirm` and `BIR_SFTP_*` environment variables. The implementation writes a durable submission record before attempting network transport. Missing credentials fail safely with a `Failed` record; uncertain SFTP failures are recorded as `Uncertain` so later automatic retries with the same idempotency key are blocked for manual review.

For BIR's current 1601C SFTP endpoint, use a non-OpenSSH backend when password auth is required:

```dotenv
# Preferred in-code transport: Rust ssh2/libssh2 SFTP, no external sftp process.
BIR_SFTP_BACKEND=native

# Fallback that mirrors the official helper most closely:
BIR_SFTP_BACKEND=winscp
BIR_WINSCP_EXE=/path/to/WinSCP.exe
BIR_WINE_CMD=wine
```

The native backend connects directly from Rust via `ssh2`, authenticates with `BIR_SFTP_USERNAME`/`BIR_SFTP_PASSWORD`, opens the remote filename, writes the encrypted payload, and treats successful close as the server acknowledgement. It intentionally does not call `fsync` because BIR's FileZilla SFTP server reports that extension as unsupported after a successful write.

The WinSCP backend remains as a compatibility fallback. It mirrors the official helper more closely than OpenSSH: WinSCP SFTP-over-SSH, binary transfer, safe SFTP v3 negotiation, and no remote `mkdir` attempt because BIR pre-creates `/1601Cv2018` and may reject `mkdir` even when upload is allowed. Credentials remain in a chmod-600 temporary WinSCP script, not argv. The default backend remains `openssh` unless `BIR_SFTP_BACKEND` is set. For OpenSSH password auth, the implementation forces `BatchMode=no`; otherwise `sftp -b` will not prompt and `sshpass` cannot supply the password.
