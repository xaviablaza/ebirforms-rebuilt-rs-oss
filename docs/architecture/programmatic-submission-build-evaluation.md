# Programmatic Submission Build Evaluation

This evaluates the implemented Rust build against `optimized-programmatic-submission-plan.md` through the first post-Milestone-4 queue/daemon slice.

## Implemented through Milestone 5 slice

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
  - SFTP transport is wired through the system `sftp` client behind `--live --confirm` and `BIR_SFTP_*` environment variables. Real credentials were not configured or used in verification.
  - Uncertain SFTP failures are mapped to `SubmissionStatus::Uncertain`, preserving manual-review semantics before retry.
  - Repeat dry-runs remain allowed so smoke tests do not create false duplicate filing blocks.
- **Milestone 5 — queue/daemon around proven CLI:** partially implemented as a CLI-driven worker loop in `crates/ebirforms-core/src/job.rs` and `ebirforms-cli` queue commands.
  - SQLite job table is created in `.ebirforms/jobs.sqlite` by default, or at `--db <jobs.sqlite>`.
  - Job statuses use the plan vocabulary: `queued`, `running`, `awaiting_receipt`, `confirmed`, `failed`, `uncertain`, `cancelled`.
  - CLI commands added: `queue`, `run-queue`, and `jobs`.
  - Queued 1601C jobs execute through dry-run transport and write durable submission records.
  - Validation/package failures become `Failed` with no retry.
  - Retryable transport failures requeue with exponential backoff.
  - Duplicate-risk and uncertain upload cases become `Uncertain` and require manual review.

## Verification run

```text
cargo test
```

Result: 16 Rust tests passed, including private captured transform tests, public render/package fixture tests, validation error behavior, durable submission records, live missing-config failure recording, dry-run repeat behavior, SQLite queue execution, no-retry validation failure, retry/backoff behavior, and uncertain-prior duplicate blocking.

```text
cargo run -q -p ebirforms-cli -- diff-fixture --form 1601C --input tests/fixtures/1601C/input.json --fixture tests/fixtures/1601C/official_encrypted.xml
```

Result:

```text
fixture match: 855 bytes, sha256 0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
```

```text
rm -f /tmp/ebirforms-m5-jobs.sqlite /tmp/ebirforms-m5-records.json
cargo run -q -p ebirforms-cli -- queue --form 1601C --input tests/fixtures/1601C/input.json --dry-run --db /tmp/ebirforms-m5-jobs.sqlite --max-attempts 3
cargo run -q -p ebirforms-cli -- run-queue --dry-run --db /tmp/ebirforms-m5-jobs.sqlite --records /tmp/ebirforms-m5-records.json --limit 1
cargo run -q -p ebirforms-cli -- jobs --db /tmp/ebirforms-m5-jobs.sqlite
python3 -m json.tool /tmp/ebirforms-m5-records.json
```

Result: queued dry-run job executed through the worker and wrote one durable submission record:

```text
job status: AwaitingReceipt
attempts: 1
submission_idempotency_key: 1601C:062026:0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
submission record status: AwaitingReceipt
payload_size: 855
payload_sha256: 0d4a1280dbf166d4b57372ff1065bed16805c8325ad6e7e6869edc1abbe9f470
remote_path: /1601C/12345678900000-1601C-062026V2#authorized@example.test#.xml
```

```text
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json
```

Result:

```text
error: submit is safe-by-default: pass --dry-run or explicitly pass --live --confirm
```

```text
cargo run -q -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --live --confirm --records /tmp/ebirforms-m4-live.json
```

Result: live mode failed safely because no real `BIR_SFTP_*` credentials were configured, and the pre-network audit record was persisted as `Failed` with a non-secret error:

```text
error: live SFTP transport requires configured BIR_SFTP_* environment variables
status: Failed
last_error: live SFTP transport requires configured BIR_SFTP_* environment variables
```

## Deliberate deviations from the optimized directory sketch

- The plan sketches separate crates (`payload`, `form-engine`, `transport`, `submission`, `db`, `daemon`). The current repo started with `ebirforms-core`, so this build adds those layers as modules inside the existing core crate instead of splitting crates immediately. This keeps the MVP small and avoids workspace churn before the API stabilizes.
- Public committed fixtures are redacted smoke fixtures, not raw taxpayer captures. The real captured private fixtures remain gitignored under `fixtures/private/1601c/` and continue to drive the official byte-compatible transform tests locally.
- Live upload is wired but not verified against BIR because no explicit real credentials were provided. The CLI therefore proves the gated path, pre-network persistence, and safe missing-config failure, not successful production filing.
- Milestone 5 is implemented as a CLI-run worker, not a resident daemon or IPC server yet. The durable SQLite job table and queue semantics are in place; long-running process supervision and IPC remain future work.

## Remaining plan gaps after this slice

- Resident daemon process, IPC endpoints, and background scheduler.
- Receipt/status tracking and reconciliation against real BIR responses.
- Desktop UI/IPC integration.
- Production credential vaulting and operator runbook for `BIR_SFTP_*` configuration.
- Additional form expansion fixtures beyond 1601C.
- Richer append-only audit log events beyond the current durable latest-state submission records and SQLite job rows.
