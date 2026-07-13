# PH Tax Forms Rust Workspace (Community Edition)

Rust workspace for a safe-by-default, data-driven form rendering, packaging, queueing, and receipt-matching prototype for Philippine tax-form workflows. This OSS distribution uses synthetic fixtures only. It is independent and unofficial: it is not affiliated with, endorsed by, sponsored by, or certified by the Philippine Bureau of Internal Revenue (BIR). It does not include, modify, redistribute, or depend on the BIR Offline eBIRForms Package. See `PROVENANCE.md`.

## Beta Testers (Enterprise Edition)

I am looking for business owners and developers who work with finance workflows to test the enterprise beta and share feedback. If you have an existing accounting team or accounting system that needs integration with this software, let me know below:

👉 Register as a beta tester:
https://docs.google.com/forms/d/e/1FAIpQLSfpW6UcHDEr0l6k3PRkaF0yC28NXxjDRhZglPxHvqVxUkekAg/viewform?usp=dialog

## License

Licensed under the Functional Source License, Version 1.1, ALv2 Future License (`FSL-1.1-ALv2`). See `LICENSE.md`.

## Current scope

- `ebirforms-core::form`: form rendering from JSON using `form.toml`, `mapping.toml`, and `template.xml`.
- `ebirforms-core::crypto`: deterministic compression/encryption/decryption transform used by tests.
- `ebirforms-core::package`: JSON → plaintext → encrypted artifact plus manifest.
- `ebirforms-core::submission`: safe-by-default dry-run/live-gated submission records.
- `ebirforms-core::job`: SQLite submission job queue.
- `ebirforms-core::profile`: local profile/settings/PIN app state.
- `ebirforms-core::receipt`: synthetic receipt parsing/matching plus local directory and packaged Himalaya mailbox polling.
- `ebirforms-cli`: command-line access to the above.

Synthetic fixtures are included for `1601C`, `2000`, `2550Q`, `0619E`, `1601EQ`, and `1702Q`. Additional forms require independently authored, redistributable templates/mappings and should be validated against public requirements or authorized operator data.

## Build with mise

This repo includes `mise.toml` to pin Rust `1.88.0`, Node `22`, and expose common build tasks.

On macOS with mise already installed, paste this into Terminal:

```bash
git clone git@github.com:xaviablaza/ebirforms-rebuilt-rs-oss.git 2>/dev/null || true
cd ebirforms-rebuilt-rs-oss
git checkout main && git pull origin main
mise trust && mise install && mise run build
./target/release/ebirforms-cli --help
```

The macOS binary will be at `./target/release/ebirforms-cli`. The build task also places a packaged `himalaya` sidecar next to the CLI at `./target/release/himalaya`, so receipt polling can run without a separate Himalaya install.

## Desktop app

A Tauri v2 + Leptos desktop shell lives under `apps/desktop`. It wraps the existing Rust core through Tauri commands and provides a focused left sidebar with `Dashboard`, `Profiles`, and `Settings`. The dashboard contains only the Tax Form Library for `1601C`, `2000`, `2550Q`, `0619E`, `1601EQ`, and `1702Q`; it requires a saved active taxpayer profile before opening a form. Package, queue/job, submission, and receipt actions are embedded in the selected form’s single-column Tax Form Flow. The desktop tasks install the Rust `trunk` web frontend builder into `target/desktop-tools/` on first run, automatically add the `wasm32-unknown-unknown` Rust target for the active mise Rust toolchain, and package Himalaya as a desktop resource/sidecar for production receipt mailbox polling.

![PH Tax Forms Desktop prototype dashboard running on Linux](docs/assets/desktop-linux-dashboard.png)

On macOS with mise already installed, paste this into Terminal to build the desktop app:

```bash
git clone git@github.com:xaviablaza/ebirforms-rebuilt-rs-oss.git 2>/dev/null || true
cd ebirforms-rebuilt-rs-oss
git checkout main && git pull origin main
mise trust && mise install
mise run desktop-build
```

Development command:

```bash
mise run desktop-dev
```

Desktop check command:

```bash
mise run desktop-check
```

SHA-256 commands:

```bash
# CLI binary
shasum -a 256 ./target/release/ebirforms-cli

# macOS app bundle archive, after zipping it for distribution
zip -r ph-tax-forms-desktop-macos.zip "apps/desktop/src-tauri/target/release/bundle/macos/PH Tax Forms Desktop (Unofficial).app"
shasum -a 256 ph-tax-forms-desktop-macos.zip
```

## Private CLI releases with embedded BIR credentials

The normal CLI reads `BIR_SFTP_*` from the runtime environment. Private release artifacts can instead embed the GitHub Actions build-time values by building with the `embed-bir-sftp-secrets` feature:

```bash
BIR_SFTP_HOST=... \
BIR_SFTP_PORT=23 \
BIR_SFTP_USERNAME=... \
BIR_SFTP_PASSWORD=*** \
cargo build --release -p ebirforms-cli --features embed-bir-sftp-secrets
```

Runtime environment variables still override the embedded fallback if they are present. The `.github/workflows/release-cli.yml` release workflow uses GitHub Actions secrets with the same names and uploads `ebirforms-cli-linux-x86_64` plus its SHA-256 file to the selected release tag.

Linux desktop builds require WebKitGTK/GTK development packages. Desktop and CLI tasks use Rust 1.88.0 via mise. macOS builds should be run on macOS with Xcode Command Line Tools installed.

Windows desktop build requirements:

- Windows 10/11
- Microsoft C++ Build Tools or Visual Studio Build Tools with the Desktop development with C++ workload
- WebView2 Runtime
- mise installed and trusted for this repo

Windows build command after prerequisites:

```powershell
git clone git@github.com:xaviablaza/ebirforms-rebuilt-rs-oss.git 2>$null
cd ebirforms-rebuilt-rs-oss
git checkout main; git pull origin main
mise trust; mise install
mise run desktop-build
```

On Windows, Tauri typically writes installers under:

```text
apps/desktop/src-tauri/target/release/bundle/msi/
apps/desktop/src-tauri/target/release/bundle/nsis/
```

## Commands

```bash
mise run test
mise run build
mise run render-sample
mise run package-sample

cargo run -p ebirforms-cli -- diff-fixture --form 1601C --input tests/fixtures/1601C/input.json --fixture tests/fixtures/1601C/synthetic_encrypted.xml
cargo run -p ebirforms-cli -- submit --form 1601C --input tests/fixtures/1601C/input.json --dry-run --records /tmp/ebirforms-submissions.json
cargo run -p ebirforms-cli -- queue --form 1601C --input tests/fixtures/1601C/input.json --dry-run --db /tmp/ebirforms-jobs.sqlite
cargo run -p ebirforms-cli -- run-queue --dry-run --db /tmp/ebirforms-jobs.sqlite --records /tmp/ebirforms-submissions.json --limit 1
cargo run -p ebirforms-cli -- jobs --db /tmp/ebirforms-jobs.sqlite
cargo run -p ebirforms-cli -- receipt-match --receipt tests/fixtures/1601C/receipt_accepted.txt --records /tmp/ebirforms-submissions.json
```

Default local state paths are under `.ebirforms/`, which is gitignored.

## Production filing configuration

Live filing is designed for authorized operators only: production packages may be built with private, operator-supplied SFTP configuration at build time, so end users do not enter SFTP settings. The public source tree intentionally has no production endpoint or username defaults. The app still defaults to `Dry run only`; switching to `Live submit to BIR` requires the Settings confirmation.

Build-time variables for a distributor/CI packaging job:

```bash
BIR_PRODUCTION_SFTP_HOST='<provided-by-authorized-operator>' \
BIR_PRODUCTION_SFTP_PORT='<provided-by-authorized-operator>' \
BIR_PRODUCTION_SFTP_USERNAME='<provided-by-authorized-operator>' \
BIR_PRODUCTION_SFTP_PASSWORD='<provided-outside-this-repository>' \
mise run desktop-build
```

Runtime `FILING_SFTP_*` variables still override the build-time values for controlled internal tests. Do not commit production hosts, usernames, passwords, private keys, or endpoint research to this repository.

## Public-source hygiene

This repository intentionally excludes private fixtures, production credentials, endpoint research, taxpayer data, official eBIRForms installers/binaries/assets, and extracted BIR package materials. See `PROVENANCE.md`, `SECURITY.md`, and `DISCLAIMER.md`.
