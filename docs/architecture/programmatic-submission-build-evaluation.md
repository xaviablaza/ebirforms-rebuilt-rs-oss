# Programmatic Submission Build Evaluation

This evaluates the implemented Rust build against `optimized-programmatic-submission-plan.md` through Milestone 4.

## Implemented through Milestone 4

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
- **Milestone 4 — durable safe transport:** implemented in `crates/ebirforms-core/src/transport.rs`, `crates/ebirforms-core/src/submission.rs`, and `ebirforms-cli submit`.
  - Dry-run submit reports and persists remote path, filename, size, payload hash, plaintext hash, and idempotency key.
  - Submitting without `--dry-run` or explicit `--live --confirm` is rejected.
  - Durable JSON `SubmissionRecord` storage is written before network transport attempts.
  - Automatic retry is blocked when a previous non-dry-run record is `Running`, `AwaitingReceipt`, `Confirmed`, or `Uncertain`.
  - Missing live SFTP configuration fails safely and records a `Failed` audit entry.
  - SFTP transport is now wired through the system `sftp` client behind `--live --confirm` and `BIR_SFTP_*` environment variables. Real credentials were not configured or used in this verification.
  - Uncertain SFTP failures are mapped to `SubmissionStatus::Uncertain`, preserving manual-review semantics before retry.
  - Repeat dry-runs remain allowed so smoke tests do not create false duplicate filing blocks.

## Verification run

```text
cargo test
```

Result: 12 Rust tests passed, including private captured transform tests, public render/package fixture tests, validation error behavior, dry-run duplicate blocking, persistent submission records, live missing-config failure recording, dry-run repeat behavior, and uncertain-prior duplicate blocking.

```text
cargo run -q -p ebirforms-cli -- diff-fixture --form 1601C --input tests/fixtures/1601C/input.json --fixture tests/fixtures/1601C/official_encrypted.xml
```

Result:

```text
fixture match: 855 bytes, sha256 0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
```

```text
rm -f /tmp/ebirforms-m4-dry.json
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --dry-run --records /tmp/ebirforms-m4-dry.json
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --dry-run --records /tmp/ebirforms-m4-dry.json
python3 -m json.tool /tmp/ebirforms-m4-dry.json
```

Result: both dry-runs succeeded and the record store contained one durable dry-run record:

```text
status: AwaitingReceipt
idempotency_key: 1601C:062026:0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
payload_size: 855
payload_sha256: 0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
remote_path: /1601C/12345678900000-1601C-062026V2#authorized@example.test#.xml
```

```text
rm -f /tmp/ebirforms-m4-live.json
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --live --confirm --records /tmp/ebirforms-m4-live.json
python3 -m json.tool /tmp/ebirforms-m4-live.json
```

Result: live mode failed safely because no real `BIR_SFTP_*` credentials were configured, and the pre-network audit record was persisted as `Failed` with a non-secret error:

```text
error: live SFTP transport requires configured BIR_SFTP_* environment variables
status: Failed
last_error: live SFTP transport requires configured BIR_SFTP_* environment variables
```

```text
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json
```

Result:

```text
error: submit is safe-by-default: pass --dry-run or explicitly pass --live --confirm
```

## Deliberate deviations from the optimized directory sketch

- The plan sketches separate crates (`payload`, `form-engine`, `transport`, `submission`). The current repo started with `ebirforms-core`, so this build adds those layers as modules inside the existing core crate instead of splitting crates immediately. This keeps the MVP small and avoids workspace churn before the API stabilizes.
- Public committed fixtures are redacted smoke fixtures, not raw taxpayer captures. The real captured private fixtures remain gitignored under `fixtures/private/1601c/` and continue to drive the official byte-compatible transform tests locally.
- Live upload is wired but not verified against BIR because no explicit real credentials were provided. The CLI therefore proves the gated path, pre-network persistence, and safe missing-config failure, not successful production filing.
- The durable record store is JSON, not SQLite. This satisfies Milestone 4's pre-network durable audit/idempotency requirement while deferring daemon-scale queue semantics.

## Remaining plan gaps after Milestone 4

- SQLite-backed daemon queue, IPC, desktop UI, and background submission lifecycle.
- Receipt/status tracking and reconciliation against real BIR responses.
- Production credential vaulting and operator runbook for `BIR_SFTP_*` configuration.
- Additional form expansion fixtures beyond 1601C.
- Richer audit log events beyond the current durable latest-state `SubmissionRecord`.
