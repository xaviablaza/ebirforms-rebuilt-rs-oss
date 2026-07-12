# AGENTS.md

## Project ownership

- Maintainer: Xavier Luis Ablaza.
- License/copyright owner: Xavier Luis Ablaza.

## eBIRForms transport strategy

- Default to the native Rust SFTP transport for BIR live uploads. Leave `BIR_SFTP_BACKEND` unset or set `BIR_SFTP_BACKEND=native`.
- The native backend uses the Rust `ssh2`/libssh2 path and has been live-tested against BIR's 1601C endpoint with an amended V4 upload. It writes the encrypted payload and treats successful remote file close as the server acknowledgement.
- Do not add an SFTP `fsync` requirement for BIR uploads. BIR's FileZilla SFTP server reports that extension as unsupported after a successful write.
- Keep WinSCP/Wine as a private/operator-supplied compatibility fallback only (`BIR_SFTP_BACKEND=winscp`, `BIR_WINSCP_EXE=/path/to/WinSCP.exe`, optional `BIR_WINE_CMD=wine`).
- Do not vendor, commit, or redistribute WinSCP binaries in this repository. WinSCP is GPL-licensed, so bundling it with a public/non-GPL distribution is not shippable without GPL redistribution compliance. The fallback may invoke a separately installed operator-provided copy, but the repo should contain only our invocation code and documentation.
- OpenSSH remains available with `BIR_SFTP_BACKEND=openssh`, but it is not the default. Password auth must force `BatchMode=no` so `sshpass` can supply the password in batch mode.

## Live submission safety

- Live submission must remain gated behind `--live --confirm` and `BIR_SFTP_*` environment variables.
- Keep durable submission records before network transport, and preserve duplicate-risk blocking for `Running`, `AwaitingReceipt`, `Confirmed`, and `Uncertain` states.
- For eBIRForms/BIR submission work, prefer CLI-first setup/preflight and final live execution via the rebuilt Rust CLI rather than proprietary helper binaries.
