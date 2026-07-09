# eBIRForms Rebuilt Rust Workspace

Local Rust implementation for reproducing the eBIRForms submission payload pipeline discovered from the Windows app investigation.

## Current scope

Implemented programmatic-submission MVP slices from `docs/architecture/optimized-programmatic-submission-plan.md`:

- `ebirforms-core::crypto::{encrypt_payload,decrypt_payload}` for the confirmed zlib + DCPcrypt-compatible AES-256 transform.
- `ebirforms-core::form` for template-first 1601C rendering from JSON using bundled `form.toml`, `mapping.toml`, and `template.xml`.
- `ebirforms-core::package` for JSON → plaintext → encrypted upload artifact plus manifest, hashes, remote path, and filename.
- `ebirforms-core::transport` for safe dry-run submission receipts, idempotency-key duplicate protection, and a gated live SFTP abstraction placeholder.
- `ebirforms-cli` commands: `encrypt`, `decrypt`, `render`, `package`, `diff-fixture`, and safe-by-default `submit --dry-run`.
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
cargo run -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --dry-run
```

Live submission is intentionally not wired to credentials yet. The CLI requires `--live --confirm`; the current `SftpTransport` returns a safe `LiveNotConfigured` error until credentials, persistence, and audit logging are implemented.
