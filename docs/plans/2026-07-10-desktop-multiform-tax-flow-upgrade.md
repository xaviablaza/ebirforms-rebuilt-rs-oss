# Desktop Multi-Form Tax Flow Upgrade Plan

Date: 2026-07-10

## Goal

Upgrade the desktop demo from a 1601C-specific workflow into a multi-form Tax Form flow that matches the original eBIRForms action model closely enough for demos while remaining synthetic and dry-run only.

## Requirements implemented

- Add BIR Form support based on provided XML form captures:
  - `2000`
  - `2550Q`
  - `0619E`
  - `1601EQ`
  - `1702Q`
  - Existing `1601C` remains supported.
- Preserve synthetic public fixture data while keeping XML field-token structure close to the captured proprietary eBIRForms plaintext XML.
- Simplify left sidebar to:
  - `Dashboard`
  - `Profiles`
  - `Settings`
- Show active profile at the bottom-left of the sidebar.
- Dashboard contains only a `Tax Form Library`; if no active saved profile exists, the library tells the operator to create and save a profile before creating a form.
- Selecting a form opens the selected Tax Form flow in a single-column form view.
- Each selected numbered BIR form renders human-readable data-entry controls for profile/period/return values and BIR line items/schedules; the UI updates the synthetic JSON payload internally and does not expose raw JSON editing to operators.
- `1601C` uses labels and section layout mapped from the January 2018 BIR PDF: fields 1–5 top strip, Part I background information, Part II computation rows 14–36, Part III payment rows 37–40, and Part IV Schedule I carry-over fields.
- Package, Jobs, Submissions, and Receipt are abstracted into the selected Tax Form flow rather than separate sidebar destinations.
- Form action buttons mirror original eBIRForms terminology:
  - `Validate` renders plaintext XML, encrypts/packages payload, shows package details, and locks the form.
  - `Edit` reopens the locked form for changes.
  - `Save` persists current form edits in the demo session.
  - `Print` is visible but disabled.
  - `Submit Final Copy`: enabled only after successful validation plus explicit final-copy confirmation; it queues and runs dry-run delivery, then leaves the submission waiting for a BIR receipt.
- Package details shown on-form:
  - Filename
  - Remote path
  - Period
  - Payload size
  - Encrypted payload SHA-256
  - Payload path
- Dry-run queue and receipt matching remain available inside the flow:
  - `Queue dry-run`
  - `Run dry-run queue`
  - `Submit Final Copy` (validated + confirmed forms only; internally queues and runs dry-run delivery)
  - `Simulate received BIR receipt`

## Source files

- Core form definitions: `crates/ebirforms-core/forms/*`
- Core form registry: `crates/ebirforms-core/src/form.rs`
- Packaging periods/filenames: `crates/ebirforms-core/src/package.rs`
- Desktop commands: `apps/desktop/src-tauri/src/commands.rs`
- Desktop invoke registration: `apps/desktop/src-tauri/src/main.rs`
- Desktop UI: `apps/desktop/frontend/src/main.rs`
- Desktop styles: `apps/desktop/frontend/src/styles.css`
- Public synthetic fixtures: `tests/fixtures/{1601C,2000,2550Q,0619E,1601EQ,1702Q}`
- Demo script: `docs/desktop-tax-form-flow-demo-script.md`

## Verification checklist

Run before considering complete:

```bash
mise x rust@1.88.0 -- cargo fmt
mise run test
mise run build
mise tasks validate
mise run desktop-check
mise run desktop-build
npm --prefix apps/desktop/frontend audit --audit-level=moderate
```

Also exercise at least one dry-run queue/receipt flow with a generated BIR-style receipt and confirm receipt matching returns `Confirmed`.
