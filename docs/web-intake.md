# Hosted assisted-filing intake

The hosted app is deliberately separate from the Tauri desktop application. It stores accounts and customer drafts in its own SQLite database and exposes only 1701Q/1702Q collection and operator review. It has no BIR upload, SFTP, queue, Himalaya, or receipt-polling API. Submission in this app means “received by the filing team,” not submitted to BIR.

## Local development on NixOS

Enter `devenv shell`, create the first operator, then start both hot-reloading processes:

```console
export EBIRFORMS_WEB_DB="$PWD/.devenv/state/web-intake.sqlite3"
export EBIRFORMS_WEB_ALLOW_EPHEMERAL_KEY=1
export EBIRFORMS_NEW_USER_PASSWORD='use-a-long-local-password'
cargo run -p ebirforms-web -- create-user operator@example.test operator
web-dev
```

Open <http://127.0.0.1:1421>. Trunk hot-reloads the Leptos/WASM frontend and proxies `/api` to Axum on port 3000. Set `EBIRFORMS_WEB_FRONTEND_PORT` to override this distinct web port. `desktop-dev` remains unchanged on port 1420 and can be run independently.

Operators create customer accounts from their workspace or with the same CLI command using the `customer` role. Passwords must be at least 12 characters. The production cookie is `Secure`, `HttpOnly`, and `SameSite=Strict`; `web-dev` alone opts out of `Secure` for plain localhost.

## VPS deployment

Set a DNS record for the VPS, copy the repository, and run:

```console
export SITE_ADDRESS=forms.example.com
export EBIRFORMS_WEB_ENCRYPTION_KEY="$(openssl rand -base64 32)"
docker compose -f compose.web.yaml build
docker compose -f compose.web.yaml run --rm \
  -e EBIRFORMS_NEW_USER_PASSWORD='replace-with-a-long-secret' \
  app ebirforms-web create-user operator@example.com operator
docker compose -f compose.web.yaml up -d
```

Caddy obtains and renews TLS certificates. Back up the `web-data` Docker volume. Do not put `BIR_SFTP_*` or email-receipt credentials in this deployment: the web process cannot use them. Account recovery is currently an operator CLI action; there is no public signup or password-reset email flow.

Persist `EBIRFORMS_WEB_ENCRYPTION_KEY` in the VPS secret manager and back it up separately from the SQLite volume. Intake payloads use authenticated XChaCha20-Poly1305 encryption with a new nonce per write. Losing or changing this key makes existing payloads unreadable; online key rotation is not yet implemented. `web-dev` deliberately uses an ephemeral local key, so its local drafts do not survive an API restart. Never enable `EBIRFORMS_WEB_ALLOW_EPHEMERAL_KEY` in production.

Administrative account commands are `list-users`, `reset-password EMAIL`, `disable-user EMAIL`, and `enable-user EMAIL`. Resetting a password or changing enabled state revokes all existing sessions for that user.

Submitted intake records can move only `Received` → `Filed` → `Receipt sent`. Deletion is an explicit hard delete. JSON export is the canonical handoff; the detail view supplies a print stylesheet for paper/PDF review.
