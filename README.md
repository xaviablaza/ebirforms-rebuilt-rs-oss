# eBIRForms Rebuilt Rust Workspace

Local Rust implementation for reproducing the eBIRForms submission payload pipeline discovered from the Windows app investigation.

## Current scope

Implemented first milestone:

- `ebirforms-core::crypto::encrypt_payload`
- `ebirforms-core::crypto::decrypt_payload`
- DCPcrypt-compatible AES-256/Rijndael CBC/tail behavior
- zlib max-compression step matching `Encrypt.exe`
- simple CLI wrapper
- private fixture tests for captured 1601C V2 artifacts

## Knowledge handoff

See:

```text
docs/session-knowledge-handoff.md
```

## Private fixtures

Captured taxpayer fixtures are intentionally gitignored:

```text
fixtures/private/1601c/plaintext-v2.xml
fixtures/private/1601c/encrypted-v2.xml
```

They are present locally on this machine so the fixture tests can verify exact byte compatibility. Do not commit real taxpayer data to a public repo.

Expected fixture hashes:

```text
plaintext-v2.xml sha256: c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713
encrypted-v2.xml sha256: 8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f
```

## Commands

```bash
cargo test
cargo run -p ebirforms-cli -- encrypt fixtures/private/1601c/plaintext-v2.xml /tmp/encrypted.xml
cargo run -p ebirforms-cli -- decrypt fixtures/private/1601c/encrypted-v2.xml /tmp/plaintext.xml
```
