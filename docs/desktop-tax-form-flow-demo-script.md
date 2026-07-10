# Desktop Tax Form Flow Demo Script

Use this talk track for the upgraded multi-form desktop demo. The app is still a safe dry-run proof path: choose a BIR form, review synthetic application data, validate/render plaintext XML, encrypt/package, queue dry-run delivery, and reconcile a simulated BIR receipt.

## Setup before the call

1. Launch with `mise run desktop-dev`, or open the built app.
2. Keep the app wide enough to show the left sidebar and dashboard.
3. Start on `Dashboard`.
4. Frame all values as synthetic demo data. Do not represent this as a live filing.

## Opening line

> “This is the desktop tax form flow. Instead of exposing package, jobs, submissions, and receipts as separate operator screens, the app now puts those actions inside the form filing workflow: choose a BIR form, validate, package, queue a dry-run, and reconcile a receipt.”

## 1. Sidebar and active profile

Show:

1. Point to the left sidebar.
2. Confirm it only contains `Dashboard` and `Profiles`.
3. Point to the active profile area at bottom left.

Say:

> “The sidebar is intentionally simple: Dashboard for filing work, Profiles for taxpayer setup. The active taxpayer profile is always visible at the bottom left so the operator knows which taxpayer context they are filing under.”

## 2. Tax Form Library

Show:

1. On `Dashboard`, point to `Tax Form Library`.
2. Choose several forms, for example `2000`, `2550Q`, `0619E`, `1601EQ`, `1702Q`, then return to `1601C` or any form you want to demo.

Say:

> “The library now supports multiple form families backed by XML templates captured from the original eBIRForms shape. These are synthetic fixtures, but the plaintext output keeps the same field-token style so it looks close enough to the proprietary application output for demo review.”

Supported demo forms:

- `1601C` — Monthly withholding on compensation
- `2000` — Documentary stamp tax
- `2550Q` — Quarterly VAT
- `0619E` — Monthly expanded withholding remittance
- `1601EQ` — Quarterly expanded withholding return
- `1702Q` — Quarterly corporate income tax

## 3. Edit and save application data

Show:

1. Select a form and filing period by choosing a tile.
2. Point to `Application data (synthetic JSON backing the XML)`.
3. Make a tiny edit if useful.
4. Click `Save`.

Say:

> “The editor is deliberately labeled synthetic. It lets us show that the application can persist current form changes in the session before validation. In a production version this can become a friendlier field-by-field form, but the backend contract is already form-code agnostic.”

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

> “Like the original eBIRForms pattern, validation locks the form. Edit reopens it for changes, and validating again regenerates the plaintext XML and encrypted payload.”

## 6. Submit Final Copy is gated by validation and confirmation

Show:

1. Point to `Print`; it remains disabled because printing is not implemented.
2. Point to the `Final copy confirmation` box.
3. Before ticking it, point to `Submit Final Copy`; it is disabled.
4. Tick `I confirm the whole form is validated, locked, and ready to submit as the final copy.`
5. Click `Submit Final Copy`.
6. Show that the app queues and runs the dry-run delivery, then displays a waiting-for-receipt state.

Say:

> “Submit Final Copy is no longer a dead button, but it is gated. The operator must first validate the whole form, review the package details, and explicitly confirm the locked final copy. Only then does the button enable. Clicking it queues the job, runs the dry-run delivery, and leaves the submission waiting for the BIR receipt confirmation.”

## 7. Receipt reconciliation completes the filing loop

Show:

1. Click `Submit Final Copy` after validation and final-copy confirmation.
2. Show the `Jobs` cards under `Submission Activity`.
3. Show `Submissions / receipt matching` in its waiting-for-receipt state.
4. Click `Simulate received BIR receipt`.
5. Confirm the submission record changes to `Confirmed`.

Say:

> “Package, jobs, submissions, and receipts are no longer separate navigation destinations. They are part of the tax form flow. Submit Final Copy creates an idempotent dry-run submission and waits for the BIR receipt; the simulated receipt is then matched back to the local submission record by filename.”

## Closing line

> “The result is a safer compliance automation loop: the operator chooses the tax form, validates and locks the XML payload, the app encrypts and queues it, and receipt matching confirms the exact submitted artifact without exposing live final-copy controls.”
