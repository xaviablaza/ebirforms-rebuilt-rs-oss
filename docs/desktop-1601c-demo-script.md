# Desktop 1601C Demo Script

Use this as the talk track for the upgraded desktop demo. The point of the demo is to show a safe, dry-run path from human-readable 1601C data entry to plaintext preview, encrypted package artifacts, queued delivery, and receipt reconciliation.

## Setup before the call

1. Launch the desktop app with `mise run desktop-dev`, or open the built app.
2. Keep the app window large enough to show the left navigation plus the main content.
3. Start from `Settings` if you want to show that the UI is live and configurable.
4. Do not use any live filing or live transport controls. Keep the story explicitly framed as a dry-run proof path.

## Opening line

Say:

> “This is the desktop proof for 1601C. The goal is not just to fill a form; it is to show the full controllable workflow: structured taxpayer data, generated BIR plaintext, encrypted package artifacts with hashes, dry-run queue delivery, and receipt reconciliation.”

## 1. Settings and theme persistence

Show:

1. Click `Settings`.
2. Click `Use dark theme`.
3. Click `Use light theme`.
4. Click `Use system theme`.

Say:

> “The desktop shell has persisted local settings. Theme changes are visible immediately, which helps prove this is a working app state, not a static mockup.”

## 2. Structured 1601C entry

Show:

1. Click `1601C`.
2. Point to the grouped sections:
   - Filing period and return type
   - Taxpayer details
   - Classification
   - Tax calculation lines.
   - Payment/credits.
   - Generated JSON audit panel.

Say:

> “The primary UI is now a business-readable 1601C form. Under the hood, these fields compile into the same JSON shape the Rust core expects, so we do not duplicate filing logic in the frontend.”

If the audience is technical, expand the generated JSON panel.

Say:

> “This audit panel is useful for developers and reviewers. It shows the exact payload passed to the rendering and packaging commands.”

## 3. Render plaintext preview

Show:

1. Click `Render plaintext preview`.
2. Show the XML/plaintext preview.
3. Point to the `Verification checklist`.

Say:

> “This step transforms the structured JSON into the BIR plaintext XML. This is the human verification point before encryption: period, TIN, taxpayer identity, ATC, and tax fields can be reviewed before anything is packaged.”

## 4. Package dry-run

Show:

1. Click `Package dry-run`.
2. Point to:
   - Filename
   - Remote path
   - Period
   - Payload size
   - Encrypted payload SHA-256
   - Payload path

Say:

> “Packaging encrypts the verified XML and creates the exact dry-run artifact plus a manifest. The hashes make the handoff auditable: we can prove which plaintext produced which encrypted payload.”

## 5. Queue dry-run delivery

Show:

1. Click `Queue dry-run job`.
2. Click `Jobs`.
3. Click `Run dry-run queue`.
4. Show the job status and attempts.

Say:

> “Delivery is modeled as an idempotent queue job. For the demo, live transport is intentionally not exposed. We can prove the package is queued and processed without risking a real submission.”

## 6. Submission record

Show:

1. Click `Submissions`.
2. Click `Refresh submissions`.
3. Show:
   - `AwaitingReceipt` status
   - Dry-run badge
   - Filename
   - Remote path
   - Payload hash
   - Attempts

Say:

> “Once the dry-run job processes, the app keeps a submission record. This becomes the local source of truth for receipt matching, audit, retries, and operator review.”

## 7. Receipt reconciliation

Show:

1. Click `Receipt`.
2. Click `Use generated BIR receipt for latest package`.
3. Click `Match receipt`.
4. Return to `Submissions` and click `Refresh submissions` if needed.
5. Show the submission status as `Confirmed`.

Say:

> “The app can parse a BIR-style receipt email, extract the filename and received date/time, and reconcile it against the local dry-run submission record. It confirms the record without resubmitting anything.”

## Closing line

Say:

> “This is the operating model we want for compliance automation: humans verify structured data and plaintext, the system packages and tracks immutable artifacts, delivery runs through a safe queue, and receipts reconcile back to the original submission record.”

## If asked what is still intentionally out of scope

Say:

> “The demo is intentionally dry-run only. Live transport and production filing controls should remain gated behind explicit environment configuration, permissions, and audit policy.”
