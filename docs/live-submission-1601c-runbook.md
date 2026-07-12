# 1601C live submission runbook

This runbook is for an explicitly authorized 1601C filing operated outside the public repository. The final upload is performed only by the CLI `submit --live --confirm` command.

This document intentionally uses placeholders. Do not commit real taxpayer data, credentials, production endpoint values, received payloads, or official package-derived materials to this repository.

## Preconditions

- The operator is authorized to file for the taxpayer.
- The operator has independently obtained any required production endpoint and credential values.
- The input JSON was produced from synthetic test data or authorized operator-provided filing data.
- Dry-run rendering, packaging, and validation have been reviewed before live mode is enabled.

Expected remote destination pattern for supported 1601C artifacts:

```text
/1601Cv2018/<TIN>-1601Cv2018-<MMYYYY>Vn#<email>#.xml
```

## One-time environment

Set secrets in your shell or private CI/distribution environment; do not commit them.

```bash
export BIR_SFTP_HOST='<provided-by-authorized-operator>'
export BIR_SFTP_PORT='<provided-by-authorized-operator>'
export BIR_SFTP_USERNAME='<provided-by-authorized-operator>'
export BIR_SFTP_PASSWORD='<provided-outside-this-repository>'
# Optional; native Rust SFTP is the default when unset.
export BIR_SFTP_BACKEND=native
# Optional if the operator has explicitly accepted the host-key policy:
export BIR_SFTP_ACCEPT_UNKNOWN_HOST=1
```

The CLI defaults to the native Rust `ssh2` SFTP backend. It also keeps explicit OpenSSH and WinSCP fallbacks for operator-local compatibility, but do not vendor those binaries in this repository.

## Prepare authorized input JSON

Use a private, operator-controlled input path. The public repository only includes synthetic fixtures.

```bash
cargo run -p ebirforms-cli -- import-xml \
  --input /path/to/authorized-operator-input.xml \
  --out /private/path/1601C-authorized-input.json \
  --email '<authorized-filer@example.com>' \
  --profile-id live-1601c-profile
```

## Preflight without upload

```bash
cargo run -p ebirforms-cli -- render \
  --form 1601C \
  --input /private/path/1601C-authorized-input.json \
  --out /tmp/ebirforms-1601c-live-preflight/plaintext.xml

cargo run -p ebirforms-cli -- package \
  --form 1601C \
  --input /private/path/1601C-authorized-input.json \
  --out /tmp/ebirforms-1601c-live-preflight/upload.xml \
  --manifest /tmp/ebirforms-1601c-live-preflight/manifest.json

cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input /private/path/1601C-authorized-input.json \
  --dry-run \
  --records .ebirforms/submissions.json
```

Check the dry run prints the expected filename/remote-path shape and `dry_run: true` before considering live mode.

## Final execution: live upload

Run this only when ready to submit the filing to BIR:

```bash
cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input /private/path/1601C-authorized-input.json \
  --live --confirm \
  --records .ebirforms/submissions.json
```

The CLI creates a durable submission record before transport, stages the encrypted payload in a temp file, uploads it with SFTP, removes the temp file, and leaves the record in `AwaitingReceipt` unless transport fails.

## Receipt matching

When the receipt email/text is available, save it locally and run:

```bash
cargo run -p ebirforms-cli -- receipt-match \
  --receipt /path/to/receipt.txt \
  --records .ebirforms/submissions.json
```

Or poll a receipt directory:

```bash
cargo run -p ebirforms-cli -- receipt-poll \
  --receipt-dir /path/to/receipts \
  --records .ebirforms/submissions.json
```
