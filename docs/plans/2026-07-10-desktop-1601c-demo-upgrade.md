# Desktop 1601C Demo Upgrade Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Upgrade the Tauri + Leptos desktop demo so themes visibly work, 1601C entry is a real form instead of a raw JSON textarea, and the demo can walk JSON → plaintext preview → encrypted package/manifest/hash → dry-run queue delivery → receipt confirmation.

**Architecture:** Keep the existing Rust core as the source of truth for rendering, packaging, hashing, queueing, and receipt matching. Add a typed Leptos form model that compiles human fields into the current `profile` / `return` / `fields` JSON shape expected by `ebirforms-core`. Improve the desktop command responses and UI panels so each stage is human-readable and demoable without exposing live submission controls.

**Tech Stack:** Rust 1.88 via mise for desktop, Tauri v2, Leptos CSR/WASM, existing `ebirforms-core` package/job/receipt modules.

---

## Source facts from current repo inspection

- Frontend entry: `apps/desktop/frontend/src/main.rs`.
- Frontend styles: `apps/desktop/frontend/src/styles.css`.
- Tauri commands: `apps/desktop/src-tauri/src/commands.rs`.
- Core 1601C rendering expects `input.profile`, `input.return`, and `input.fields`.
- Proprietary/reference fixture: `/home/vettel/ebirforms-rebuilt-rs-proprietary/tests/fixtures/1601C/input.json`.
- 1601C mapped field names from `mapping.toml` include:
  - Period/amendment: `txtMonth`, `txtYear`, `AmendedRtn_1`, `AmendedRtn_2`, `txtSheets`.
  - Taxpayer: `txtTIN1`, `txtTIN2`, `txtTIN3`, `txtBranchCode`, `txtRDOCode`, `txtTaxpayerName`, `txtAddress`, `txtAddress2`, `txtZipCode`, `txtTelNum`.
  - Classification: `CatAgent_P`, `CatAgent_G`, `SpecialTax_1`, `SpecialTax_2`, `selTreaty`, `txtATC`, `TaxWithheld_1`, `TaxWithheld_2`.
  - Tax summary: `txtTax14` through `txtTax36`, with labels to be added in UI.
  - Payment/credit rows: `txtAgency37`, `txtNumber37`, `txtDate37`, `txtAmount37`, etc.
  - Page 2/schedule: `txtPg2TIN1`, `txtPg2TIN2`, `txtPg2TIN3`, `txtPg2BranchCode`, `txtPg2TaxpayerName`, `sched1:txtTotal1`, `txtCurrentPage`, `txtMaxPage`, `txtLineBus`.
- Current receipt parser only handles internal key/value fixtures (`Receipt-ID`, `Status`, `Filename`, `Form`, `Period`, `Received-At`). It should also parse the BIR email format shown by the user (`File name`, `Date received by BIR`, `Time received by BIR`).

## Acceptance criteria

1. Settings theme buttons visibly change the app immediately:
   - `Use system theme` follows `prefers-color-scheme`.
   - `Use dark theme` forces dark styling.
   - `Use light theme` forces light styling.
   - The chosen theme persists through the backend settings store and reloads on app startup.
2. The `1601C` route is no longer a raw JSON editor as the primary UI.
   - It shows grouped, labeled form inputs with sensible synthetic defaults.
   - It can still expose an expandable/generated JSON panel for audit/debug.
3. `Render plaintext preview` builds JSON from form fields and calls existing `render_1601c`.
   - The UI shows a readable XML preview and a verification checklist.
4. `Package dry-run` builds JSON from form fields and calls existing `package_1601c`.
   - The UI shows filename, remote path, payload size, plaintext SHA-256, encrypted payload SHA-256, payload artifact path, manifest path.
5. `Queue dry-run job` queues the same built JSON.
   - The UI shows job status and the `Jobs` screen can run the dry-run queue.
6. Receipt matching accepts the sample BIR email body, extracts filename/date/time, infers `1601C` and period from filename, and confirms the matching dry-run submission record.
7. Verification commands pass:
   - `mise run test`
   - `mise run build`
   - `mise tasks validate`
   - `mise run desktop-check`
   - `mise run desktop-build`
   - `npm --prefix apps/desktop/frontend audit --audit-level=moderate`
8. Demo instructions in `README.md` match the real UI labels and paths.

---

### Task 1: Add receipt parser support for real BIR confirmation email text

**Objective:** Let the demo receipt text provided by the user match a dry-run submission without forcing internal fixture labels.

**Files:**
- Modify: `crates/ebirforms-core/src/receipt.rs`
- Test fixture/update if useful: `tests/fixtures/1601C/receipt_accepted.txt`

**Step 1: Add failing parser test**

Add a test that parses this shape:

```text
SUBJECT: "Tax Return Receipt Confirmation"
FROM: ebirforms-noreply@bir.gov.ph
This confirms receipt of your submission with the following details subject to validation by BIR:
File name: 010961925000-1601Cv2018-012026V1.xml
Date received by BIR: 15 April 2026
Time received by BIR: 03:10 PM
```

Expected parsed metadata:
- `filename = 010961925000-1601Cv2018-012026V1.xml`
- `form_code = 1601C`
- `period_mmYYYY = 012026`
- `status_text = RECEIVED`
- `received_at` can be a stable string like `15 April 2026 03:10 PM`.
- `receipt_id` can be deterministic, e.g. `BIR-010961925000-1601Cv2018-012026V1.xml`.

**Step 2: Run focused test and verify failure**

Run:

```bash
cargo test -p ebirforms-core receipt::tests::parses_bir_receipt_confirmation_email
```

Expected: FAIL because required internal fields are missing.

**Step 3: Implement fallback parser**

Keep existing key/value fixture support, but in `parse_receipt` add fallback logic:
- Read normalized `file_name` when `filename` is absent.
- Infer `form_code` with regex/string search for `1601C` or `1601Cv2018` in filename.
- Infer period from filename pattern `-MMYYYY` or `-MMYYYYVn` before `.xml`.
- Treat presence of `File name` + `Date received by BIR` as status `RECEIVED`.
- Compose `received_at` from date and time fields.

Avoid adding a regex crate unless needed; simple string scanning is enough.

**Step 4: Run tests**

```bash
cargo test -p ebirforms-core receipt
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/ebirforms-core/src/receipt.rs
git commit -m "feat: parse BIR receipt confirmation emails"
```

---

### Task 2: Make theme state real in the frontend

**Objective:** Theme buttons should apply visible light/dark/system styling immediately and persist via `update_settings`.

**Files:**
- Modify: `apps/desktop/frontend/src/main.rs`
- Modify: `apps/desktop/frontend/src/styles.css`

**Step 1: Add a frontend theme signal**

In `App`, add:
- `theme: RwSignal<String>` defaulting to `system`.
- `effective_theme` class on `<main>` or `<body>` via app root class names.
- A startup call to `app_snapshot` that reads `settings.theme` and initializes the signal.

**Step 2: Add a dedicated theme command wrapper**

Do not use the generic `run_command` only. Create `set_theme_preference(theme_name)` that:
1. Immediately updates the frontend signal for responsive UX.
2. Calls `update_settings`.
3. Rolls back or shows error if the backend rejects it.

**Step 3: Pass current theme into Settings**

Update `Settings` props:
- `theme: ReadSignal<String>`
- `set_theme_preference: impl Fn(&'static str)`

Show active state on selected theme button.

**Step 4: Add CSS variables**

In `styles.css`:
- Define light variables on `:root` / `.theme-light`.
- Define dark variables on `.theme-dark`.
- Define system variables under `@media (prefers-color-scheme: dark)` for `.theme-system`.
- Replace hard-coded backgrounds/text/borders with variables.

Minimum variables:
- `--bg`
- `--surface`
- `--surface-muted`
- `--text`
- `--muted`
- `--border`
- `--primary`
- `--primary-hover`
- `--sidebar-bg`
- `--sidebar-text`
- `--pre-bg`

**Step 5: Verify**

```bash
mise run desktop-check
```

Expected: frontend builds and backend checks.

**Step 6: Commit**

```bash
git add apps/desktop/frontend/src/main.rs apps/desktop/frontend/src/styles.css
git commit -m "feat: apply desktop theme preferences"
```

---

### Task 3: Replace raw 1601C JSON-first screen with typed form model

**Objective:** Make `1601C` demo entry understandable to a business user.

**Files:**
- Modify: `apps/desktop/frontend/src/main.rs`

**Step 1: Add typed form struct**

Create a `Form1601CInput` struct in frontend with string/bool fields that map to the known fixture fields:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Form1601CInput {
    profile_id: String,
    tin: String,
    email: String,
    month: String,
    year: String,
    amended: bool,
    amendment_number: String,
    tax_withheld_agent: bool,
    sheets: String,
    atc: String,
    rdo_code: String,
    taxpayer_name: String,
    registered_address: String,
    zip_code: String,
    telephone: String,
    category_private: bool,
    special_tax_rate: bool,
    treaty_code: String,
    tax_14: String,
    tax_15: String,
    tax_16: String,
    tax_17: String,
    tax_18: String,
    tax_19: String,
    tax_20_other: String,
    tax_20: String,
    tax_21: String,
    tax_22: String,
    tax_23: String,
    tax_24: String,
    tax_25: String,
    tax_26: String,
    tax_27: String,
    tax_28: String,
    tax_29_other: String,
    tax_29: String,
    tax_30: String,
    tax_31: String,
    tax_32: String,
    tax_33: String,
    tax_34: String,
    tax_35: String,
    tax_36: String,
    payment_agency_37: String,
    payment_number_37: String,
    payment_date_37: String,
    payment_amount_37: String,
    schedule_total_1: String,
    line_of_business: String,
}
```

Start with these fields; do not attempt to model every obscure payment row in the first pass unless the UI remains readable.

**Step 2: Add defaults from proprietary fixture**

Implement `Default` for `Form1601CInput` using safe synthetic values from `/home/vettel/ebirforms-rebuilt-rs-proprietary/tests/fixtures/1601C/input.json`:
- TIN: `123-456-789-00000`
- email: `authorized@example.test`
- month/year: `01` / `2026` for the demo receipt compatibility, or keep `06/2026` and generate receipt dynamically from package filename.
- ATC: `WW010`
- RDO: `044`
- taxpayer: `AUTHORIZED TEST TAXPAYER`
- tax amounts: `0.00`

**Step 3: Add conversion function**

Add:

```rust
fn form_1601c_to_json(input: &Form1601CInput) -> serde_json::Value
```

It must produce:
- `profile.tin`, `profile.email`, `profile.profile_id`
- `return.period.month`, `return.period.year`, `return.is_amended`, `return.amendment_number`
- `fields.*` exactly as `mapping.toml` expects.

TIN splitting:
- Strip non-digits.
- `txtTIN1`: first 3 digits.
- `txtTIN2`: next 3.
- `txtTIN3`: next 3.
- `txtBranchCode`: remaining digits padded/truncated to branch code display.
- Page 2 TIN fields should match page 1, not the current synthetic `123/123/123` mismatch.

Boolean mapping:
- `AmendedRtn_1 = amended`, `AmendedRtn_2 = !amended`.
- `TaxWithheld_1` / `TaxWithheld_2` based on whether tax was withheld.
- `CatAgent_P` / `CatAgent_G` based on private/government category.
- `SpecialTax_1` / `SpecialTax_2` based on special rate flag.

**Step 4: Rewrite `Form1601C` component**

Group fields into cards/fieldsets:
1. Filing period and return type.
2. Taxpayer details.
3. Classification.
4. Tax calculation lines.
5. Payment/credits.
6. Generated JSON audit panel.

Replace primary raw textarea with labeled inputs/selects/checkboxes.

Keep a collapsible `<details>` with generated JSON so technical viewers can see the exact payload.

**Step 5: Wire actions to generated JSON**

Update buttons:
- `Render plaintext preview` calls `render_1601c` with `form_1601c_to_json(&form.get())`.
- `Package dry-run` calls `package_1601c` with same JSON.
- `Queue dry-run job` calls `queue_1601c_dry_run` with same JSON.

**Step 6: Verify**

```bash
mise run desktop-check
```

Expected: PASS.

**Step 7: Commit**

```bash
git add apps/desktop/frontend/src/main.rs
git commit -m "feat: add structured 1601C form entry"
```

---

### Task 4: Improve result panels for preview, package, jobs, and submissions

**Objective:** Make each demo stage readable instead of dumping raw JSON everywhere.

**Files:**
- Modify: `apps/desktop/frontend/src/main.rs`
- Modify: `apps/desktop/frontend/src/styles.css`

**Step 1: Add typed response structs in frontend**

Add Deserialize structs mirroring backend responses:
- `PackagePreviewResponse`
- `SubmissionManifestResponse`
- `JobResponse`
- `SafeSubmissionRecordResponse`

Keep raw string fallback for unexpected responses.

**Step 2: Split preview/package state**

Current `package_preview` stores both XML plaintext and package JSON. Split into:
- `plaintext_preview: String`
- `package_preview: Option<PackagePreviewResponse>`
- `package_raw: String`

**Step 3: Improve `PackagePreview` UI**

Required fields:
- Filename
- Remote path
- Period
- Payload size
- Encrypted payload SHA-256 short/full
- Payload path

Use a `<dl class="details">` layout already present in CSS. The historical live SFTP path uploads only the encrypted payload filename; no manifest artifact is needed in Package Details.

**Step 4: Improve Jobs UI**

Show a list/table-like card for each job:
- Job ID
- Form
- Mode
- Status
- Attempts
- Next run time
- Last error

Retain raw JSON in `<details>`.

**Step 5: Improve Submissions UI**

Show each submission:
- Filename
- Status (`AwaitingReceipt` / `Confirmed`)
- Dry-run badge
- Remote path
- Payload SHA-256 short
- Attempts
- Receipt status

**Step 6: Verify**

```bash
mise run desktop-check
```

Expected: PASS.

**Step 7: Commit**

```bash
git add apps/desktop/frontend/src/main.rs apps/desktop/frontend/src/styles.css
git commit -m "feat: show readable desktop workflow results"
```

---

### Task 5: Add a one-click synthetic receipt generator for demo safety

**Objective:** Let the presenter simulate the official receipt against the actual filename generated by the package/queue flow.

**Files:**
- Modify: `apps/desktop/frontend/src/main.rs`

**Step 1: Build receipt text from latest package/submission**

Add a frontend helper:

```rust
fn sample_bir_receipt_for_filename(filename: &str) -> String
```

Output should match the user-provided format:

```text
SUBJECT: "Tax Return Receipt Confirmation"
FROM: ebirforms-noreply@bir.gov.ph
This confirms receipt of your submission with the following details subject to validation by BIR:
File name: {filename}
Date received by BIR: 15 April 2026
Time received by BIR: 03:10 PM
...
```

**Step 2: Add Receipt screen button**

If a latest package/submission filename is known, add:
- `Use generated BIR receipt for latest package`

This fills the textarea with matching filename so `Match receipt` can succeed.

**Step 3: Verify manually through Tauri command calls or UI**

Run:

```bash
mise run desktop-check
```

Expected: PASS.

**Step 4: Commit**

```bash
git add apps/desktop/frontend/src/main.rs
git commit -m "feat: generate matching demo receipt text"
```

---

### Task 6: Update README demo path and presenter script

**Objective:** Give Xavi exact click instructions for a prospect demo.

**Files:**
- Modify: `README.md`

**Step 1: Add desktop demo walkthrough**

Add a section after Desktop app commands:

```markdown
### Desktop demo walkthrough

1. Launch: `mise run desktop-dev` or open the built app.
2. Click `Settings` → `Use dark theme`, then `Use light theme`, then `Use system theme`.
3. Click `1601C`.
4. Review Filing period and return type, Taxpayer details, Classification, Tax calculation lines, and Payment/credits fields.
5. Click `Render plaintext preview`; explain this is the XML the human verifies before encryption.
6. Click `Package dry-run`; show filename, remote path, payload size, plaintext SHA-256, encrypted payload SHA-256, payload path, and manifest path.
7. Click `Queue dry-run job`.
8. Click `Jobs` → `Run dry-run queue`; show status moves to AwaitingReceipt.
9. Click `Submissions` → `Refresh submissions`; show dry-run record and payload hash.
10. Click `Receipt` → `Use generated BIR receipt for latest package` → `Match receipt`; show status becomes Confirmed.
```

**Step 2: Commit**

```bash
git add README.md
git commit -m "docs: add desktop demo walkthrough"
```

---

### Task 7: Full verification and proof artifact

**Objective:** Prove the app builds and the flow is demo-ready.

**Files:**
- No source files expected, unless a verification bug is found.

**Step 1: Run full checks**

```bash
mise run test
mise run build
mise tasks validate
mise run desktop-check
mise run desktop-build
npm --prefix apps/desktop/frontend audit --audit-level=moderate
```

Expected: all pass.

**Step 2: Optionally capture screenshot/video**

On Linux CI/VPS, use the existing README screenshot pattern if display packages are available. On macOS, run the built app and capture screenshots of:
- Theme dark/light proof.
- 1601C structured form.
- Package result hashes.
- Confirmed receipt submission.

**Step 3: Final commit/push**

```bash
git status --short
git push origin main
```

---

## Demo click script after implementation

1. Open the app.
2. Click `Settings`.
3. Click `Use dark theme`.
   - Say: “Theme preference is persisted locally; this is not a static mock.”
4. Click `Use light theme`.
5. Click `Use system theme`.
6. Click `1601C`.
7. Show the form sections:
   - Filing period and amendment.
   - Taxpayer details.
   - Classification/ATC.
   - Tax calculation lines.
   - Payment/credits.
   - Generated JSON audit panel.
8. Click `Render plaintext preview`.
   - Say: “This is JSON transformed into the BIR plaintext XML; this is where the human verifies before encryption.”
9. Click `Package dry-run`.
   - Show filename, remote path, payload size, plaintext SHA-256, encrypted payload SHA-256, payload path, manifest path.
   - Say: “Packaging encrypts the verified XML and produces the exact artifact plus checksums.”
10. Click `Queue dry-run job`.
    - Say: “The delivery system is queued/idempotent; live transport is not exposed in the demo.”
11. Click `Jobs`.
12. Click `Run dry-run queue`.
    - Show the job has been processed.
13. Click `Submissions`.
14. Click `Refresh submissions`.
    - Show `AwaitingReceipt`, filename, remote path, dry-run badge, hash.
15. Click `Receipt`.
16. Click `Use generated BIR receipt for latest package`.
17. Click `Match receipt`.
    - Show submission status becomes `Confirmed`.
    - Say: “The system reconciles a BIR receipt email against the local submission record without resubmitting.”

## Notes / risks

- The user-provided receipt filename format `010961925000-1601Cv2018-012026V1.xml` differs from the current package filename pattern `{tin}-{form_code}-{period_mmYYYY}{amendment_suffix}#{email}#.xml`. For the demo to match, either generate the receipt from the actual package filename or update the package filename pattern if proprietary evidence confirms the official format. First implementation should generate receipt text from the actual package filename to keep the flow coherent.
- Keep all live filing paths hidden or gated. The demo should explicitly label `dry-run` everywhere.
- Avoid copying private/proprietary fixtures into OSS. Use the field names/structure and synthetic values only.
