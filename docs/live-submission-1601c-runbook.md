# 1601C live submission runbook

This runbook is for an explicitly authorized 1601C amended filing. The final upload is performed only by the CLI `submit --live --confirm` command.

## Evidence used

The 1601C template is derived from the proprietary eBIRForms 1601Cv2018 plaintext XML structure. For the captured June 2026 amended V2 zero-tax filing, the rebuilt CLI now reproduces both:

- plaintext XML byte-for-byte
- encrypted upload payload byte-for-byte

Expected live remote destination pattern:

```text
/1601Cv2018/<TIN>-1601Cv2018-<MMYYYY>Vn#<email>#.xml
```

Observed transport from the proprietary app:

```text
SFTP over SSH
Host: ftp2.birgovph.com
Port: 23
Remote directory: /1601Cv2018/
```

## One-time environment

Set secrets in your shell; do not commit them.

```bash
export BIR_SFTP_HOST=ftp2.birgovph.com
export BIR_SFTP_PORT=23
export BIR_SFTP_USERNAME='<provided-by-BIR-or-app-config>'
export BIR_SFTP_PASSWORD='<provided-by-BIR-or-app-config>'
# Optional; this is already the default when unset.
export BIR_SFTP_BACKEND=native
# Optional if the host key is not already pinned:
export BIR_SFTP_ACCEPT_UNKNOWN_HOST=1
```

The CLI defaults to the native Rust `ssh2` SFTP backend. It also keeps explicit OpenSSH and WinSCP fallbacks for operator-local compatibility, but do not vendor those binaries in this repository.

## Import official XML to CLI input JSON

If starting from an official eBIRForms plaintext XML:

```bash
cargo run -p ebirforms-cli -- import-xml \
  --input /path/to/<TIN>-1601Cv2018-062026V2.xml \
  --out fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --email '<authorized-filer@example.com>' \
  --profile-id live-1601c-profile
```

## Preflight without upload

```bash
cargo run -p ebirforms-cli -- render \
  --form 1601C \
  --input fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --out /tmp/ebirforms-1601c-live-preflight/plaintext.xml

cargo run -p ebirforms-cli -- package \
  --form 1601C \
  --input fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --out /tmp/ebirforms-1601c-live-preflight/upload.xml \
  --manifest /tmp/ebirforms-1601c-live-preflight/manifest.json

cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --dry-run \
  --records .ebirforms/submissions.json
```

Check the dry run prints:

```text
dry_run: true
status: AwaitingReceipt
remote_path: /1601Cv2018/<TIN>-1601Cv2018-062026V2#<authorized-filer@example.com>#.xml
```

## Final execution: live upload

Run this only when ready to submit the amended filing to BIR:

```bash
cargo run -p ebirforms-cli -- submit \
  --form 1601C \
  --input fixtures/private/1601C-062026-amended-v2-from-cli-import.json \
  --live --confirm \
  --records .ebirforms/submissions.json
```

The CLI will create a durable submission record before transport, stage the encrypted payload in a temp file, upload it with SFTP, remove the temp file, and leave the record in `AwaitingReceipt` unless transport fails.

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
