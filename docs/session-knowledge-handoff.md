# eBIRForms Rebuild / Programmatic Submission — Session Knowledge Handoff

Created: 2026-07-09T04:21:05+00:00

This file consolidates the working knowledge discovered in this investigation so the Rust/Leptos rebuild can proceed without depending on chat history.

## Objective

Build a headless, API-driven daemon plus native cross-platform client that can construct, encrypt, and submit valid Philippine BIR eBIRForms returns without human intervention.

Initial target form captured/reproduced:

```text
Withholding Tax - Compensation, Form 1601C
Form code/version observed: 1601Cv2018
Period fixture: 06/2026
```

## Product requirements from user

Minimum app/client requirements:

- Leptos can be used as the base.
- Settings page:
  - security, privacy, global application preferences
  - Master PIN: 4-digit PIN to unlock app
  - if Master PIN forgotten, OS login password required
  - Master Authenticator App: 6-digit TOTP code support
  - Enable Profile PINs
  - Hide Tax Profile from Sidebar, gated by Profile PINs
  - Global Toggle Hotkey: `Win + Shift + [Key]`, reassignable to letter/number
- Background tasks & job queue:
  - system daemon jobs
  - retry queues
  - application logs
- Dark mode / light mode.
- Taxpayer Profile creation fields:
  1. TIN: 4 boxes, 14 digits total: 3 + 3 + 3 + 5
  2. RDO key-value pair, e.g. `044 -> Taguig and Pateros`
  3. Taxpayer Type: Individual, Corporation, Partnership, Cooperative, Estate, Trust
  4. Tax Classification for Individual: Purely Compensation, Self-Employed / Professional, Mixed Income
  5. Line of Business for Corporation, Partnership, Estate, Trust
  6. Cooperative Tax Treatment: Exempt, Taxable, Mixed
  7. EOPT Tier: Micro, Small, Medium, Large
  8. Taxpayer Name
  9. Registered Address
  10. Zip code
  11. Phone number
  12. Email address
  13. Business start date
  14. VAT registered taxpayer / Has Employees / Expanded Withholding Agent checkboxes
  15. Excise tax liabilities
  16. Email connection:
      - Gmail via Google Account OAuth2
      - or app password for Gmail/Outlook/Yahoo
      - verify connection before activating email tracking
- Tax form library minimum:
  - Corporate Income Tax, Form 1702
  - Corporate Income Tax, Form 1702Q
  - Value Added Tax, Form 2550Q
  - Withholding Tax - Compensation, Form 1601C
  - Withholding Tax - Compensation, Form 1604C
  - Withholding Tax - Expanded/Others, Form 0619E
  - Withholding Tax - Expanded/Others, Form 1601EQ
  - Withholding Tax - Expanded/Others, Form 1604E

## Windows VM / investigation environment notes

Known Windows VM user/path:

```text
User: Xavi
App install/capture root: C:\eBIRForms
Capture folder: C:\Users\Xavi\Documents\ebirforms-1601C-capture
```

Important eBIRForms directories observed:

```text
C:\eBIRForms\savefile
C:\eBIRForms\IAF_RDO_Copy
C:\eBIRForms\IAF_RDC_Copy
C:\eBIRForms\IAF_RDO_Archive
C:\eBIRForms\logfile
C:\Users\Xavi\AppData\Local\Temp
```

## Captured local file lifecycle

The official eBIRForms app flow for 1601C is now confirmed:

```text
BIRForms.exe
  -> writes readable saved pseudo-XML to C:\eBIRForms\savefile
  -> writes/copies plaintext staging pseudo-XML to C:\eBIRForms\IAF_RDO_Copy
  -> calls Encrypt.exe <staging-file>
  -> Encrypt.exe compresses/encrypts the staging file in-place
  -> app copies/renames encrypted bytes to #email# filename
  -> ebfSFTP.exe invokes WinSCP
  -> WinSCP uploads to the BIR SFTP backend
```

Timestamped watcher captured exact before/after sequence for a V2 amended 1601C submission:

```text
11:35:32.5547
<TIN>-1601Cv2018-062026V2.xml
5571 bytes
plaintext field dump

11:35:32.6830
<TIN>-1601Cv2018-062026V2.xml
956 bytes
binary encrypted/compressed output

11:35:32.7488
<TIN>-1601Cv2018-062026V2#<email>#.xml
956 bytes
final upload artifact
```

The post-encryption `V2.xml` and final `V2#email#.xml` were byte-for-byte identical.

## Plaintext saved/staging format

The readable form files are not strict XML documents. They are pseudo-XML field dumps with many top-level `<div>` records, e.g.:

```xml
<?xml version='1.0'?>
<div>frm1601c:txtMonth=06frm1601c:txtMonth=</div>
<div>frm1601c:txtYear=2026frm1601c:txtYear=</div>
```

The field/value encoding pattern is approximately:

```text
<div>{field_id}={value}{field_id}=</div>
```

Known amended-return behavior:

```text
frm1601c:AmendedRtn_1: false -> true
frm1601c:AmendedRtn_2: true  -> false
```

Filename gains `Vn` amendment suffix:

```text
<TIN>-1601Cv2018-062026.xml      # original style
<TIN>-1601Cv2018-062026V1.xml    # amended version 1
<TIN>-1601Cv2018-062026V2.xml    # amended version 2
```

## Confirmed network/transport behavior

WinSCP log confirmed:

```text
Protocol: SFTP over SSH
Host: ftp2.birgovph.com
Port: 23
Username observed in log: carlo
Remote working directory: /
Remote form directory: /1601Cv2018/
Transfer mode: binary
Upload result: success
```

Remote destination pattern:

```text
/1601Cv2018/<TIN>-1601Cv2018-<MMYYYY>[Vn]#<email>#.xml
```

The app also queries a dispatcher/config endpoint before upload:

```text
/tinDispatcherSFTP.php?t=<TIN-prefix>&f=1601Cv2018&v=7.9.6.0
```

Do not assume the same credentials/endpoints apply to every form/version without verifying. Store secrets outside git.

## Binary bundle analyzed

User provided compressed binaries:

```text
Encrypt.exe        489,452 bytes
WinSCP.exe      24,060,560 bytes
WinSCPnet.dll      162,152 bytes
cFTPSend.exe       335,360 bytes
ebfSFTP.exe          9,216 bytes
```

SHA-256:

```text
Encrypt.exe   429337f44f84b93cd1095df48c8f3265e5ede7c646d1b48d9b80f4f92de74d2c
ebfSFTP.exe   c6ba25014d30a11b97d9d90c3b87f2f0c13d35ef6188ea5c086f48b3933d297f
cFTPSend.exe  5d3dbda56e3ffffefb23f2fd46a5af0c0decc389d70921c453c3f813bb806262
WinSCP.exe    bd11fd16014ce10d456fda42dabc79369d15074137edbda70dbeb201212735d7
WinSCPnet.dll dab8f3fe073f157f609ad288b33402ab96e94005d8b56ff0fa3bd8c0c27750d3
```

`Encrypt.exe` is a 32-bit FreePascal/Lazarus console program with symbols/debug information preserved. It statically links:

```text
zstream / zlib compression
DCPcrypt 2.0.4.1
TDCP_rijndael
TDCP_sha256
```

Important symbols:

```text
P$ENCRYPT_COMPRESS$ANSISTRING$ANSISTRING$$BOOLEAN
P$ENCRYPT_READFILEANDENCRYPT$ANSISTRING$$BOOLEAN
P$ENCRYPT_COMPRESSANDENCRYPT$ANSISTRING$$BOOLEAN
PASCALMAIN
```

`ebfSFTP.exe` is a .NET Framework 4.7.2 console app wrapping WinSCP .NET automation. Relevant strings:

```text
WinSCP.Session
WinSCP.SessionOptions
WinSCP.TransferOptions
PutFiles
set_Protocol
set_HostName
set_PortNumber
set_UserName
set_Password
set_SshHostKeyPolicy
set_TransferMode
```

PDB path string:

```text
D:\code_repository\github_repo\EBIRFormsSFTP\ebfSFTP\ebfSFTP\obj\Debug\ebfSFTP.pdb
```

`cFTPSend.exe` appears to be an older/native FTP fallback using Synapse-style FTP/socket code (`TFTPSend`, `USER`, `PASS`, `SITE`, `UsePassive`). The captured active path was SFTP via `ebfSFTP.exe` + WinSCP, not `cFTPSend.exe`.

## Exact Encrypt.exe behavior

Function chain:

```text
PASCALMAIN
  -> ParamStr(1)
  -> CompressAndEncrypt(file)

CompressAndEncrypt(file)
  -> Compress(file, "tmp.zip")
  -> DeleteFile(file)
  -> RenameFile("tmp.zip", file)
  -> ReadFileAndEncrypt(file)

ReadFileAndEncrypt(file)
  -> encryptedTemp = same directory + "tmpAES.tmp"
  -> open input file
  -> open tmpAES.tmp output
  -> create TDCP_rijndael
  -> InitStr(passphrase, TDCP_sha256)
  -> EncryptStream(input, tmpAES.tmp, input.Size)
  -> delete original file
  -> rename tmpAES.tmp back to original file
```

Embedded temporary filenames:

```text
tmp.zip
tmpAES.tmp
```

Embedded passphrase:

```text
T0081gP45sy0rd-To+R3m3m63r!@4/<>
```

Exact transform:

```text
plaintext pseudo-XML
  -> zlib compress, max compression / level 8 or 9 equivalent
  -> AES-256/Rijndael via DCPcrypt InitStr(passphrase, SHA256)
  -> DCPcrypt block cipher default mode: CBC
  -> DCPcrypt no-padding partial-tail behavior
  -> binary .xml file
```

DCPcrypt `InitStr` key derivation:

```text
key = SHA256(passphrase)
```

DCPcrypt IV behavior with nil IV:

```text
IV = AES_encrypt_block(16 zero bytes)
CV = IV
```

DCPcrypt CBC full-block encryption:

```text
cipher_block = AES_encrypt(plain_block XOR CV)
CV = cipher_block
```

DCPcrypt final partial-block behavior:

```text
stream = AES_encrypt(CV)
tail_cipher = tail_plain XOR stream[0:tail_len]
```

No PKCS#7 padding. Ciphertext length equals compressed length.

## Fixture verification already achieved in Python

Working script:

```text
/home/vettel/ebirforms-investigation/ebirforms_transform.py
```

Usage:

```bash
python3 ebirforms_transform.py encrypt plaintext.xml encrypted.xml
python3 ebirforms_transform.py decrypt encrypted.xml plaintext.xml
```

Verified local reproduction:

```text
captured V2 plaintext size:   5571 bytes
captured V2 plaintext sha256: c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713

reproduced encrypted size:    956 bytes
reproduced encrypted sha256:  8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f

official encrypted size:      956 bytes
official encrypted sha256:    8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f
```

Decryption verification:

```text
decrypted reproduced encrypted file -> 5571 bytes
sha256: c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713
```

Compression finding for exact V2 plaintext:

```text
zlib level 8: 956 bytes, sha256 af0f9e56ce7b383ad03cfe527089c96e8cc1dadd8b2b1d050fda9e9d545fea76
zlib level 9: 956 bytes, sha256 af0f9e56ce7b383ad03cfe527089c96e8cc1dadd8b2b1d050fda9e9d545fea76
```

After AES/DCPcrypt encryption, final encrypted SHA-256 is:

```text
8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f
```

## Rust implementation target

Core API should provide:

```rust
pub fn encrypt_payload(plaintext: &[u8]) -> Vec<u8>;
pub fn decrypt_payload(ciphertext: &[u8]) -> Result<Vec<u8>, Error>;
```

Conceptual implementation:

```rust
let compressed = zlib_compress_level_9(plaintext);
let key = sha256(b"T0081gP45sy0rd-To+R3m3m63r!@4/<>");
let encrypted = dcpcrypt_aes256_cbc_encrypt_preserve_tail(&compressed, &key);
```

Recommended Rust crates:

```text
sha2
aes
cipher
flate2
thiserror
```

Important: do not use normal CBC mode crate with padding. Implement the DCPcrypt-compatible CBC/tail behavior directly using AES block encrypt/decrypt primitives.

## Initial local Rust repo plan

Suggested repo path:

```text
/home/vettel/ebirforms-rebuilt-rs
```

Suggested modules:

```text
crates/ebirforms-core/src/crypto.rs      # DCPcrypt-compatible transform
crates/ebirforms-core/src/fields.rs      # pseudo-XML field dump builder/parser, later
crates/ebirforms-core/src/forms/form1601c.rs
crates/ebirforms-cli/src/main.rs         # fixture encrypt/decrypt CLI, optional
fixtures/1601c/plaintext-v2.xml
fixtures/1601c/encrypted-v2.xml
```

First milestone:

- create Rust workspace
- implement `ebirforms-core::crypto`
- add tests using captured fixture files
- verify:

```text
encrypt(plaintext-v2.xml) SHA-256 == 8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f
decrypt(encrypted-v2.xml) SHA-256 == c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713
```

## Next milestones after crypto

1. Build `1601C` field-dump generator from typed Rust structs.
2. Validate generated pseudo-XML byte-for-byte or field-for-field against captured savefile/staging examples.
3. Implement SFTP submission using a Rust SFTP/SSH client or controlled external client.
4. Implement dispatcher lookup behavior carefully; avoid hard-coding secrets.
5. Repeat capture/reproduction for:
   - 1702
   - 1702Q
   - 2550Q
   - 1604C
   - 0619E
   - 1601EQ
   - 1604E
6. Build daemon job queue and retry model.
7. Build Leptos UI/settings/profile management around the core library.

## Security notes

- Do not commit real taxpayer PII, TINs, email addresses, SFTP passwords, or submitted return contents into public repos.
- Current fixture paths in this local environment include real-looking TIN/email values in filenames. For long-term repo hygiene, either keep the repo private/local or sanitize fixture filenames/content while preserving byte-level crypto fixtures in an encrypted/private fixture store.
- The embedded `Encrypt.exe` passphrase is part of the legacy client behavior and is required to reproduce file compatibility. Treat it as compatibility logic, not an authentication secret.
