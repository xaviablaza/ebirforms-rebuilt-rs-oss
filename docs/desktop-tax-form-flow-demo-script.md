# Desktop Tax Form Flow Demo Script

Use this talk track for the upgraded multi-form desktop demo. The app defaults to a safe dry-run proof path: choose a BIR form, review synthetic application data, validate/render plaintext XML, encrypt/package, queue dry-run delivery, and reconcile a simulated or mailbox-detected BIR receipt. Live BIR submission is available only from Settings after an explicit confirmation dialog.

## Setup before the call

1. Launch with `mise run desktop-dev`, or open the built app.
2. Keep the app wide enough to show the left sidebar, dashboard library, and single-column form flow.
3. Start on `Dashboard`.
4. Frame all values as synthetic demo data. Do not represent this as a live filing.

## Opening line

> “This is the desktop tax form flow. Instead of exposing package, jobs, submissions, and receipts as separate operator screens, the app now puts those actions inside the form filing workflow: choose a BIR form, validate, package, queue a dry-run, and reconcile a receipt.”

## 1. Sidebar and active profile

Show:

1. Point to the left sidebar.
2. Confirm it contains `Dashboard`, `Profiles`, and the restored `Settings` tab.
3. Point to the active profile area at bottom left.

Say:

> “The sidebar is intentionally simple: Dashboard for the tax form library, Profiles for taxpayer setup, and Settings for theme and lock controls. The active taxpayer profile is always visible at the bottom left so the operator knows which taxpayer context they are filing under.”

## 2. Tax Form Library

Show:

1. On `Dashboard`, point to `Tax Form Library` as the only dashboard content.
2. If no taxpayer profile is saved, show the warning telling the operator to create and save a profile first.
3. Save or choose a profile under `Profiles`, then return to `Dashboard`.
4. Choose several forms, for example `2000`, `2550Q`, `0619E`, `1601EQ`, `1702Q`, then return to `1601C` or any form you want to demo. Each click opens a single-column `Tax Form Flow`.

Say:

> “The dashboard is now only the Tax Form Library. It will not let an operator create a tax form until a taxpayer profile has been saved. Once a profile exists, selecting a form opens that form’s single-column Tax Form Flow. The library supports multiple form families backed by independently authored synthetic XML templates and redistributable mappings.”

Supported demo forms:

- `1601C` — Monthly withholding on compensation
- `2000` — Documentary stamp tax
- `2550Q` — Quarterly VAT
- `0619E` — Monthly expanded withholding remittance
- `1601EQ` — Quarterly expanded withholding return
- `1702Q` — Quarterly corporate income tax

## 3. Edit and save application data

Show:

1. In the selected form’s single-column `Tax Form Flow`, point to `BIR Form <code> data entry`.
2. For `1601C`, show that the screen now follows the physical January 2018 BIR PDF: fields 1–5 across the top, Part I background information, Part II computation rows 14–36, Part III payment details, and Part IV Schedule I adjustment carry-over.
3. Make a tiny edit if useful, for example Item 14 Total Amount of Compensation, Item 25 Total Taxes Withheld, RDO code, or amended return.
4. Click `Save`.

Say:

> “The form view is intentionally single-column: actions, final-copy confirmation, form data, package details, XML preview, jobs, submissions, and receipt matching all flow downward on the selected tax form. For 1601-C, operators now see the same numbered labels and parts as the physical BIR form. The UI maps those human fields back to the same synthetic JSON keys that render into XML, so nobody has to edit raw JSON directly.”

## 4. Validate locks the form and packages it

Show:

1. Click `Validate`.
2. Point out the `Validated / locked` badge.
3. Point to the plaintext XML preview.
4. Point to `Package details`.

Say:

> “Validate is the equivalent of preparing the final copy. It renders the plaintext XML, encrypts the payload, and locks the form for submission readiness. Package details are shown on the form itself: filename, remote path, period, payload size, encrypted payload SHA-256, and payload path.”

Package details to call out:

- Filename
- Remote path
- Period
- Payload size
- Encrypted payload SHA-256
- Payload path

## 5. Edit reopens the form

Show:

1. Click `Edit`.
2. Confirm the badge returns to `Editable`.
3. Optionally click `Validate` again after edits.

Say:

> “Validation locks the form after generating the synthetic filing payload. Edit reopens it for changes, and validating again regenerates the plaintext XML and encrypted payload.”

## 6. Submit Final Copy is gated by validation, confirmation, and Settings mode

Show:

1. Point to `Print`; it remains disabled because printing is not implemented.
2. Point to the `Final copy confirmation` box.
3. Before ticking it, point to `Submit Final Copy`; it is disabled.
4. Tick `I confirm the whole form is validated, locked, and ready to submit as the final copy.`
5. Click `Submit Final Copy` while Settings remains on `Dry run only`.
6. Show that the app queues and runs the dry-run delivery, then displays a waiting-for-receipt state.
7. Optional production-path proof: open `Settings`, click `Live submit to BIR`, and show the confirmation dialog. Cancel it unless the demo has real `FILING_SFTP_*` production credentials and filing approval.

Say:

> “Submit Final Copy is no longer a dead button, but it is gated. The operator must validate the whole form, review package details, explicitly confirm the locked final copy, and keep or change the Settings submission mode. The default is dry run. Switching to Live submit to BIR requires a confirmation dialog and configured production SFTP credentials.”

## 7. Receipt reconciliation completes the filing loop

Show:

1. Click `Submit Final Copy` after validation and final-copy confirmation.
2. Show the `Jobs` cards under `Submission Activity`.
3. Show `Submissions / receipt matching` in its waiting-for-receipt state.
4. Click `Simulate received BIR receipt` for the offline proof path, or click `Check receipt mailbox (Himalaya)` when Himalaya is installed/configured against the production receipt mailbox.
5. Confirm the submission record changes to `Confirmed`.

Say:

> “Package, jobs, submissions, and receipts are no longer separate navigation destinations. They are part of the tax form flow. Submit Final Copy creates an idempotent submission record and waits for the BIR receipt; the receipt can be simulated for demos or pulled from the receipt mailbox through Himalaya, then matched back to the local submission record by filename, form, and period.”

## 8. Settings regression check

Show:

1. Click `Settings` in the sidebar.
2. Confirm `Dry run only` is selected by default.
3. Click `Live submit to BIR` and show the confirmation dialog; cancel unless real filing is approved.
4. Toggle system/dark/light theme.
5. Optionally set a 4-digit PIN and lock/unlock the app.

Say:

> “Settings is back in the navigation. Submission mode defaults to Dry run only; live BIR submission requires an explicit confirmation dialog and production filing credentials. Theme preference and the simple lock screen remain available while the filing workflow stays focused on Dashboard, Profiles, and Settings.”

## Closing line

> “The result is a safer compliance automation loop: the operator chooses the tax form, validates and locks the XML payload, the app encrypts and queues it in dry-run or explicitly-confirmed live mode, and receipt matching confirms the exact submitted artifact from either a simulated receipt or the production mailbox.”
