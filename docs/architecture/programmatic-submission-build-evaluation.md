# Programmatic Submission Build Evaluation

This evaluates the implemented Rust build against `optimized-programmatic-submission-plan.md` as of the current milestone commit.

## Implemented in this build

- **Milestone 1 — 1601C transform:** implemented and fixture-tested in `crates/ebirforms-core/src/crypto.rs`.
  - Private captured fixture test still proves `plaintext-v2.xml -> encrypted-v2.xml` byte-for-byte.
  - Expected private ciphertext length/hash remain `956` bytes and `8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f`.
- **Milestone 2 — template renderer:** implemented in `crates/ebirforms-core/src/form.rs`.
  - Bundled 1601C definition files live under `crates/ebirforms-core/forms/1601C/`.
  - Public redacted fixture proves `tests/fixtures/1601C/input.json -> official_plaintext.xml` byte-stably.
  - Missing mapped fields return `FormError::MissingValue` instead of malformed XML.
- **Milestone 3 — package builder:** implemented in `crates/ebirforms-core/src/package.rs` and exposed through `ebirforms-cli package`.
  - Builds plaintext, encrypted upload bytes, filename, remote path, and JSON manifest.
  - Public redacted fixture proves `input.json -> official_encrypted.xml` byte-stably.
- **Milestone 4 — safe transport dry-run:** partially implemented in `crates/ebirforms-core/src/transport.rs` and `ebirforms-cli submit`.
  - Dry-run transport reports remote path, filename, size, hash, and idempotency key.
  - Submitting without `--dry-run` or `--live --confirm` is rejected.
  - Live SFTP is intentionally gated with `LiveNotConfigured` until credentials, persistence, and audit logging are implemented.
  - In-memory duplicate idempotency protection is covered by tests; persistent idempotency records are still future work.

## Verification run

```text
cargo test
```

Result: 8 Rust tests passed, including private captured transform tests, public render/package fixture tests, validation error behavior, and dry-run duplicate blocking.

```text
cargo run -q -p ebirforms-cli -- diff-fixture --form 1601C --input tests/fixtures/1601C/input.json --fixture tests/fixtures/1601C/official_encrypted.xml
```

Result:

```text
fixture match: 855 bytes, sha256 0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
```

```text
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json
```

Result:

```text
error: submit is safe-by-default: pass --dry-run or explicitly pass --live --confirm
```

```text
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --live --confirm
```

Result:

```text
error: live SFTP transport requires --live --confirm and configured credentials
```

## Deliberate deviations from the optimized directory sketch

- The plan sketches separate crates (`payload`, `form-engine`, `transport`, `submission`). The current repo started with `ebirforms-core`, so this build adds those layers as modules inside the existing core crate instead of splitting crates immediately. This keeps the MVP small and avoids workspace churn before the API stabilizes.
- Public committed fixtures are redacted smoke fixtures, not raw taxpayer captures. The real captured private fixtures remain gitignored under `fixtures/private/1601c/` and continue to drive the official byte-compatible transform tests locally.
- SFTP live upload is not implemented yet. The safety gate and transport abstraction are in place, but actual network code should wait for persistent idempotency/audit storage and explicit credential handling.

## Remaining plan gaps

- Persistent `SubmissionRecord` storage before network calls.
- Durable idempotency and `Uncertain` state across process restarts.
- Real SFTP upload to `ftp2.birgovph.com:23` behind `--live --confirm`.
- Daemon queue, SQLite jobs, IPC, desktop UI, and receipt tracking.
- Additional form expansion fixtures beyond 1601C.
