---
name: add-bir-form-1701q
description: Add BIR Form 1701Q to the PH Tax Forms desktop app and Rust core using a clean XML template, official PDF-derived layout parameters, and optional user-supplied normalized labels.
tags: [ebirforms, bir, tax-forms, desktop, tauri, rust]
---

# Add BIR Form 1701Q to the Desktop App

Use this skill when adding **BIR Form 1701Q** (Quarterly Income Tax Return for Individuals, Estates and Trusts) to this repository's Rust core, CLI, tests, and Tauri/Leptos desktop app.

The expected source inputs are:

1. A clean, redistributable **1701Q plaintext XML shape** or authorized sanitized capture that can become `template.xml` + `mapping.toml`.
2. The official **BIR 1701Q PDF** used only as layout/label/source-of-truth parameters for the physical desktop renderer.
3. Optional user-provided **normalized form labels**. Prefer these labels over OCR guesses when they map unambiguously to official PDF item numbers and XML keys.

Do **not** commit taxpayer data, production credentials, official eBIRForms binaries, extracted proprietary assets, or private endpoint research.

## Target files

Create or update these repo paths:

- `crates/ebirforms-core/forms/1701Q/form.toml`
- `crates/ebirforms-core/forms/1701Q/mapping.toml`
- `crates/ebirforms-core/forms/1701Q/template.xml`
- `tests/fixtures/1701Q/input.json`
- `tests/fixtures/1701Q/synthetic_plaintext.xml`
- `crates/ebirforms-core/src/form.rs`
- `crates/ebirforms-core/src/receipt.rs`
- `apps/desktop/frontend/src/main.rs`
- `README.md` and any demo/planning docs that enumerate supported forms

Optional helper artifacts may live under `docs/form-research/1701Q/` if they are clean-room notes, PDF text extraction output, or crosswalk tables. Do not commit the official PDF unless licensing is explicitly cleared.

## Implementation workflow

### 1. Establish sources and provenance

- Record the exact official PDF URL in `form.toml` as `pdf_url` only after verifying the URL resolves to the intended 1701Q revision.
- Treat the PDF as the UI/layout source of truth: item numbers, sections, human-readable labels, checkbox/radio grouping, and printed order.
- Treat the XML capture/template as the serialization source of truth: XML keys, ordering, filename shape, required internal fields, and encoded checkbox values.
- If Xavi supplies normalized labels, keep a small crosswalk that records:
  - `item`
  - normalized label
  - official PDF label/text snippet
  - `xml_key`
  - `json_path`
  - visual section/row

### 2. Build the form assets

Create `crates/ebirforms-core/forms/1701Q/` with:

- `form.toml`
  - `code = "1701Q"`
  - `display_name = "Quarterly Income Tax Return for Individuals, Estates and Trusts"` unless the verified PDF revision says otherwise.
  - `category = "income_tax_individual"`
  - `frequency = "quarterly"`
  - Choose `period_format` from the actual filename/XML convention. Do not assume from other forms.
    - Existing repo conventions include `YYYYQn` for `1702Q` and `MMYYYYQn` for `2550Q`.
  - `remote_directory` and `filename_pattern` must match the XML/BIR convention discovered for 1701Q.
  - Add `[[sections]]` and `[[sections.fields]]` from the PDF/normalized-label crosswalk.
- `mapping.toml`
  - TOML keys are XML keys; values are JSON paths.
  - Deduplicate XML keys before writing the file. Captured forms can contain repeated labels or continuation-page headers; TOML cannot contain duplicate keys.
- `template.xml`
  - Preserve XML order, namespace/key names, CRLF-sensitive content conventions, and internal serialization fields.
  - Replace dynamic values with `{{xml_key}}` placeholders that exactly match `mapping.toml` keys.

### 3. Create safe synthetic fixtures

Create `tests/fixtures/1701Q/input.json` and `tests/fixtures/1701Q/synthetic_plaintext.xml`.

Fixture rules:

- Use clearly synthetic taxpayer names, TINs, contact details, addresses, email addresses, and amounts.
- Preserve realistic form semantics: quarter, year, amended return flag, ATC, taxpayer type, deduction method, tax-computation rows, credits/payments, penalties, and payment details when present in the PDF/XML.
- Include XML-only/internal serialization fields in JSON/template when needed for byte-stable rendering, but do not expose them as printed desktop form controls unless the PDF shows them.

### 4. Register the form in Rust core

Update `crates/ebirforms-core/src/form.rs`:

- Add a `"1701Q" => Self::from_static(...)` arm in `FormDefinition::builtin`.
- Add `"1701Q"` to the multi-form fixture test list in `renders_new_pdf_mapped_forms_from_human_readable_layouts`.
- Keep the existing invariant: every physical layout field's `xml_key` must map to the same `json_path` in `mapping.toml`.

Run a focused render check:

```bash
cargo test -p ebirforms-core form::tests::renders_new_pdf_mapped_forms_from_human_readable_layouts
cargo run -p ebirforms-cli -- render --form 1701Q --input tests/fixtures/1701Q/input.json --out /tmp/1701q-plaintext.xml
diff -u tests/fixtures/1701Q/synthetic_plaintext.xml /tmp/1701q-plaintext.xml
```

### 5. Extend filename and receipt parsing

Update `crates/ebirforms-core/src/receipt.rs` wherever supported form codes are enumerated or regexed.

- Add `1701Q` to supported filename/receipt patterns.
- Add tests for the real 1701Q filename shape once known.
- Confirm the inferred period format matches the chosen `form.toml` `period_format`.

Suggested focused check:

```bash
cargo test -p ebirforms-core receipt::tests::infers_form_and_period_from_supported_bir_filenames
```

### 6. Add desktop app support

Update `apps/desktop/frontend/src/main.rs` by following existing `1702Q`/`2550Q` patterns:

- `include_str!("../../../../tests/fixtures/1701Q/input.json")`
- Add `1701Q` to the tax form library list with a concise description.
- Add physical renderer branches and helper label/section/title mappings.
- Render top-strip controls as printed-form controls, not raw XML fields:
  - quarter/year boxes or radio groups
  - amended-return Yes/No
  - TIN/branch segmented fields
  - ATC choice/code
  - taxpayer/background information
  - computation/payment/schedule rows
- Keep JSON/XML key names hidden from operators.
- Keep `Validate`, `Edit`, `Save`, disabled `Print`, and gated `Submit Final Copy` behavior consistent with other forms.

Run desktop checks after wiring:

```bash
mise run desktop-check
```

### 7. Update CLI/docs supported-form lists

Update any user-visible supported-form enumerations:

- `README.md`
- `docs/desktop-tax-form-flow-demo-script.md`
- `docs/plans/*` only if the plan is still current and enumerates supported forms
- CLI help/tests only if forms are listed explicitly there

Avoid claiming live 1701Q filing support until live transport destination, filename, and receipt behavior have been verified by an authorized operator.

## Verification checklist

Before committing a 1701Q implementation, run at minimum:

```bash
cargo fmt --all
cargo test -p ebirforms-core form::tests::renders_new_pdf_mapped_forms_from_human_readable_layouts
cargo test -p ebirforms-core receipt::tests::infers_form_and_period_from_supported_bir_filenames
cargo run -p ebirforms-cli -- render --form 1701Q --input tests/fixtures/1701Q/input.json --out /tmp/1701q-plaintext.xml
diff -u tests/fixtures/1701Q/synthetic_plaintext.xml /tmp/1701q-plaintext.xml
mise run test
mise run desktop-check
```

If a full desktop build is required for handoff:

```bash
mise run desktop-build
```

## Pitfalls

- Do not derive labels from XML keys alone. Use the official PDF and Xavi's normalized labels where available.
- Do not expose internal XML state fields in the physical form renderer.
- Do not assume 1701Q has the same period or filename shape as 1702Q just because both are quarterly income tax forms.
- Do not commit real taxpayer data from a capture. Redact/synthesize before fixtures land in the repo.
- Do not fabricate official PDF URLs or remote directories. Leave them as researched TODOs in a branch if they have not been verified.
- Do not treat dry-run packaging as live BIR acceptance.

## Commit guidance

Use Xavi's repo identity for commits:

```bash
git config user.name "Xavi Ablaza"
git config user.email "xavi@hostari.com"
git status --short
git diff --check
git add docs/skills/add-bir-form-1701q/SKILL.md <implementation files>
git commit -m "Document 1701Q desktop form workflow"
```
