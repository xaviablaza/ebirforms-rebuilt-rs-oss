# Optimized eBIRForms Programmatic Submission Plan

> Updated assumption: Form 1601C reverse engineering is complete enough to implement a byte-compatible Rust reproducer. The core optimization is to treat future form support as a data/template problem wherever possible: capture official plaintext pseudo-XML, map canonical fields into it, then reuse the same compression/encryption/package/submission pipeline.

## Objective

Build a Rust-first, headless submission daemon and cross-platform Leptos/Tauri client for authorized BIR filings. The system must construct valid plaintext eBIRForms payloads, transform them with the official-compatible payload transform, submit them through the observed transport, and track receipts.

## Key Simplification

The original plan treated each form as a large custom implementation. The optimized plan separates the system into two layers:

1. **Universal Submission Pipeline**
   - plaintext pseudo-XML input
   - canonical filename generation
   - zlib compression
   - DCPcrypt-compatible AES-256 transform
   - SFTP upload package
   - local submission state machine
   - email receipt tracking

2. **Form Adapters**
   - field schema
   - validation rules
   - official plaintext XML template
   - mapping from canonical app data into template fields
   - eligibility metadata

This means new forms should usually require adding:

```text
forms/<form_code>/schema.toml
forms/<form_code>/template.xml
forms/<form_code>/mapping.toml
forms/<form_code>/validation.rs or validation.toml
fixtures/<form_code>/official_plaintext.xml
fixtures/<form_code>/official_encrypted.xml
```

Only forms with genuinely different transport or transform behavior should require new Rust pipeline code.

---

## Confirmed 1601C Pipeline Baseline

Use this as the golden path for MVP and regression tests.

```text
official plaintext pseudo-XML
  -> zlib compression, level 9-compatible
  -> DCPcrypt Rijndael/AES-256
     key = SHA256("T0081gP45sy0rd-To+R3m3m63r!@4/<>")
     mode = DCPcrypt CBC behavior
     IV = AES_encrypt(16 zero bytes)
     no PKCS#7 padding
     final partial tail = XOR with AES_encrypt(CV)
  -> binary bytes stored with .xml filename
  -> SFTP/SSH upload via ftp2.birgovph.com:23
  -> remote directory /<formCode>/
  -> filename <TIN>-<formCode>-<MMYYYY>[Vn]#<email>#.xml
```

The immediate implementation priority is not more UI. It is locking this pipeline behind fixture tests so every future form can reuse it safely.

---

## Optimized Architecture

```text
crates/
  core/                 taxpayer profile, enums, RDO/EOPT, shared validation primitives
  form-engine/          form registry, schemas, XML template rendering, mappings
  payload/              1601C-confirmed transform: zlib + DCPcrypt-compatible AES tail mode
  transport/            SFTP transport, mock transport, dry-run transport
  submission/           state machine, idempotency, package builder, receipt correlation
  db/                   SQLite migrations and repositories
  daemon/               queue runner, IPC, email polling, logs
  email/                Gmail OAuth, IMAP app password, receipt parser
apps/
  cli/                  first-class MVP proving tool
  desktop/              Tauri v2 + Leptos shell after CLI works
tests/
  fixtures/
    1601C/
      input.json
      official_plaintext.xml
      official_encrypted.xml
      official_filename.txt
    form-template-smoke/
docs/
  reverse-engineering/
  architecture/
  plans/
tools/
  windows-lab/
  payload-diff/
  fixture-import/
```

### Important Change

Make `apps/cli` the first deliverable, not the desktop app. The CLI should prove the whole objective before UI complexity enters:

```bash
ebirforms-cli package --form 1601C --input tests/fixtures/1601C/input.json --out /tmp/out.xml
ebirforms-cli diff-fixture --form 1601C --input tests/fixtures/1601C/input.json
ebirforms-cli submit --form 1601C --input authorized-return.json --dry-run
ebirforms-cli submit --form 1601C --input authorized-return.json --live --confirm
```

---

## Form Expansion Strategy: Template-First

### Form Definition Contract

Each supported form gets a definition folder:

```text
crates/form-engine/forms/1601C/
  form.toml
  template.xml
  mapping.toml
  validation.toml
  examples/minimal.json
```

Example `form.toml`:

```toml
code = "1601C"
version = "v2018"
display_name = "Withholding Tax - Compensation"
category = "withholding_compensation"
frequency = "monthly"
remote_directory = "/1601C/"
filename_pattern = "{tin}-{form_code}v2018-{period_mmYYYY}{amendment_suffix}#{email}#.xml"
requires_employees = true
requires_expanded_withholding_agent = false
requires_vat_registered = false
```

Example `mapping.toml`:

```toml
[taxpayer]
"txtTIN1" = "profile.tin.part1"
"txtTIN2" = "profile.tin.part2"
"txtTIN3" = "profile.tin.part3"
"txtBranchCode" = "profile.tin.part4"
"txtRDOCode" = "profile.rdo_code"
"txtTaxpayerName" = "profile.taxpayer_name"
"txtAddress" = "profile.registered_address"
"txtZipCode" = "profile.zip_code"
"txtEmail" = "profile.email_address"

[return]
"txtMonth" = "return.period.month"
"txtYear" = "return.period.year"
"chkAmended" = "return.is_amended"
```

### Why This Matters

For the next forms, do not hand-code a new serializer first. Instead:

1. Generate a blank/representative form in official eBIRForms.
2. Capture the plaintext XML before `Encrypt.exe` overwrites it.
3. Redact PII into a fixture.
4. Identify variable fields.
5. Add `template.xml` with placeholders.
6. Add `mapping.toml` from app model to placeholders.
7. Run `render -> transform -> compare` against official encrypted output.

This reduces form expansion from “build a custom module” to “add a fixture and mapping,” with Rust code only for complex validations/calculations.

---

## Revised Milestones

## Milestone 1 — Lock 1601C Transform as a Golden Fixture

**Goal:** byte-for-byte reproduce official 1601C encrypted upload from plaintext.

Deliverables:
- `crates/payload` implements DCPcrypt-compatible transform.
- Fixture test proves official plaintext -> official encrypted bytes.
- CLI command can encrypt/decrypt/test fixture locally.

Acceptance:
- `cargo test -p payload` passes.
- Fixture ciphertext hash equals captured official upload hash.
- Output length matches compressed/encrypted artifact length exactly.

## Milestone 2 — Template Renderer for 1601C Plaintext XML

**Goal:** generate official-compatible plaintext pseudo-XML from structured JSON.

Deliverables:
- `crates/form-engine` with template loader and mapping engine.
- 1601C `template.xml`, `mapping.toml`, `form.toml`.
- `input.json -> plaintext.xml` fixture test.

Acceptance:
- Rendered plaintext is byte-stable.
- Official fixture comparison passes after normalizing known volatile fields.
- Missing required fields produce validation errors, not malformed XML.

## Milestone 3 — End-to-End 1601C Package Builder

**Goal:** turn JSON into final `.xml` upload artifact and filename.

Deliverables:
- package builder combines form rendering + payload transform + filename rules.
- package manifest records hashes, form code, period, generated time, profile ID.
- CLI `package` command.

Acceptance:
- `input.json -> encrypted upload artifact` matches fixture.
- filename matches official convention.
- no PII/secrets emitted in normal logs.

## Milestone 4 — Submission Transport Dry-Run and Manual Live Gate

**Goal:** implement transport without accidental duplicate filings.

Deliverables:
- SFTP transport abstraction.
- Mock/dry-run transport enabled by default.
- Live submit requires explicit `--live --confirm` and authorized input.
- idempotency key and payload hash stored before network call.

Acceptance:
- dry-run logs remote path, filename, size, and hash.
- live mode is impossible to trigger accidentally.
- uncertain prior upload blocks automatic retry.

## Milestone 5 — Queue/Daemon Around Proven CLI

**Goal:** daemonize the proven package/submit flow.

Deliverables:
- SQLite job table.
- job statuses: queued, running, awaiting_receipt, confirmed, failed, uncertain, cancelled.
- retry rules: network only; validation no retry; uncertain requires manual review.
- IPC endpoints for jobs and submissions.

Acceptance:
- queued 1601C job executes through mock transport.
- network failure retries with backoff.
- duplicate-risk scenario becomes `Uncertain`, not resubmitted.

## Milestone 6 — Minimal Leptos/Tauri Desktop

**Goal:** UI shell over existing daemon capabilities.

Deliverables:
- app shell, dashboard, profiles, form library, jobs, logs, settings.
- taxpayer profile creation.
- 1601C form entry can queue a submission.
- light/dark/system theme.
- master PIN basic lock.

Acceptance:
- Windows app launches.
- profile can be created and secured.
- 1601C can be queued and observed in jobs page.
- daemon logs visible.

## Milestone 7 — Email Receipt Tracking

**Goal:** close the loop from submitted to confirmed.

Deliverables:
- Gmail OAuth verify.
- Gmail/Outlook/Yahoo IMAP app-password verify.
- receipt parser and matcher.
- polling job updates submission status.

Acceptance:
- connection cannot activate until verified.
- receipt fixture matches correct submission.
- confirmed status includes receipt metadata.

## Milestone 8 — Add More Forms by Fixture

**Goal:** expand from 1601C without rewriting the pipeline.

Order:
1. 1601EQ or 0619E, because they likely share withholding patterns.
2. 2550Q, because VAT adds a different calculation shape.
3. 1604C/1604E annual summaries.
4. 1702Q/1702 corporate income tax.

For each form:
- capture official plaintext XML
- capture official encrypted artifact
- add form folder definition
- add JSON input fixture
- add render fixture test
- add package fixture test
- add UI route generated from schema or metadata

Acceptance for each new form:
- package generation is fixture-tested.
- form-specific validations are encoded.
- no changes needed in `payload` unless evidence proves transform differs.

---

## Revised Data Model Priority

Implement only what the pipeline needs first:

1. `TaxpayerProfile`
2. `Tin`
3. `RdoCode`
4. `TaxPeriod`
5. `FormCode`
6. `SubmissionPackage`
7. `SubmissionRecord`
8. `Job`

Defer broad security/preferences until the CLI and daemon prove submission.

Security UI requirements remain valid, but they are not on the critical path for programmatic submission. Build them after the package/transport path is fixture-proven.

---

## Revised MVP Scope

### MVP A — Proven 1601C CLI

Must include:
- 1601C JSON input model.
- 1601C plaintext template rendering.
- official-compatible transform.
- package filename generation.
- dry-run transport.
- fixture tests.

No desktop UI, no TOTP, no email yet.

### MVP B — Safe Live 1601C Submission

Must include:
- SFTP submit.
- explicit live confirmation gate.
- idempotency record.
- uncertain-state duplicate protection.
- audit log.

### MVP C — Daemon + Desktop Shell

Must include:
- local queue.
- job page.
- taxpayer profile page.
- 1601C page.
- settings basics: master PIN, theme.

### MVP D — Receipt Tracking

Must include:
- Gmail OAuth verify.
- IMAP app-password verify.
- receipt matching.

### MVP E — Form Expansion Pack

Add the remaining forms by template/fixture:
- 0619E
- 1601EQ
- 2550Q
- 1604C
- 1604E
- 1702Q
- 1702

---

## Implementation Rule of Thumb

When adding a form, ask:

1. Can official eBIRForms produce a plaintext XML for this form?
2. Does `Encrypt.exe` produce a byte-compatible artifact with the same transform?
3. Does the filename/remote directory follow the same convention?
4. Are validations expressible in `validation.toml`?
5. Are calculations simple enough for template expressions, or do they need Rust code?

Only write custom Rust form code when the answer to 4 or 5 requires it.

---

## Critical Risks and Controls

### Risk: false confidence from 1601C only

Control: every new form needs its own official plaintext and encrypted fixture. Do not assume transform or directory until verified once.

### Risk: duplicate filings

Control: idempotency records, payload hashes, receipt polling, and `Uncertain` state before retrying ambiguous uploads.

### Risk: logging sensitive taxpayer data

Control: structured redaction by default; raw payload debug mode must be explicit and local-only.

### Risk: template drift after BIR updates forms

Control: version each form definition, store official fixture hashes, add `form_version` to package manifest.

### Risk: 4-digit PIN is weak

Control: Argon2id, attempt throttling, OS-bound secret, optional TOTP. Never collect OS login password directly; use OS-native auth and secure storage.

---

## Next Concrete Tasks

1. Freeze `crates/payload` around the confirmed 1601C transform.
2. Add `apps/cli package` and `apps/cli diff-fixture`.
3. Convert captured 1601C plaintext into a redacted `template.xml` with placeholders.
4. Add `mapping.toml` and `input.json` for 1601C.
5. Make `cargo test` prove `input.json -> plaintext -> encrypted artifact`.
6. Add dry-run transport and package manifest.
7. Only then add daemon queue and desktop UI.

This ordering maximizes the chance of achieving the real objective: programmatic valid submission, not just a polished data-entry app.
