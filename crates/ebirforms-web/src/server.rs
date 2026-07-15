#![allow(dead_code)]

use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Extension, Path as AxumPath, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::{distributions::Alphanumeric, Rng};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower_http::services::{ServeDir, ServeFile};

use crate::{Intake, PortalError, SaveResult, Session, SubmissionResult};
use leptos::prelude::{expect_context, provide_context};

const SESSION_COOKIE: &str = "ebirforms_session";
const SESSION_SECONDS: i64 = 60 * 60 * 12;
const LOGIN_WINDOW_SECONDS: i64 = 15 * 60;
const LOGIN_MAX_FAILURES: i64 = 5;
const LOGIN_BLOCK_SECONDS: i64 = 60;

#[derive(Clone)]
pub struct Store(Arc<StoreInner>);

struct StoreInner {
    connection: Mutex<Connection>,
    encryption_key: [u8; 32],
}

#[derive(Clone)]
struct AppState {
    store: Store,
}

#[derive(Debug, Clone, Serialize)]
struct Actor {
    id: i64,
    email: String,
    role: String,
    csrf_token: String,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: String,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

fn error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (
        status,
        Json(ApiError {
            error: message.into(),
        }),
    )
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}
fn reference() -> String {
    format!("EBIR-{}-{}", now(), token()[..8].to_ascii_uppercase())
}
fn session_hash(raw: &str) -> String {
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}

fn key_from_env() -> Result<[u8; 32], String> {
    if let Ok(encoded) = std::env::var("EBIRFORMS_WEB_ENCRYPTION_KEY") {
        let decoded = BASE64
            .decode(encoded.trim())
            .map_err(|_| "EBIRFORMS_WEB_ENCRYPTION_KEY must be base64".to_string())?;
        return decoded.try_into().map_err(|_| {
            "EBIRFORMS_WEB_ENCRYPTION_KEY must decode to exactly 32 bytes".to_string()
        });
    }
    if std::env::var("EBIRFORMS_WEB_ALLOW_EPHEMERAL_KEY")
        .ok()
        .as_deref()
        == Some("1")
    {
        let mut key = [0_u8; 32];
        rand::thread_rng().fill(&mut key);
        eprintln!("WARNING: using an ephemeral web payload key; existing drafts will be unreadable after restart");
        return Ok(key);
    }
    Err("EBIRFORMS_WEB_ENCRYPTION_KEY is required (32 random bytes, base64 encoded)".into())
}

fn encrypt_payload(key: &[u8; 32], value: &Value) -> Result<String, String> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let mut nonce = [0_u8; 24];
    rand::thread_rng().fill(&mut nonce);
    let plaintext = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| "payload encryption failed".to_string())?;
    Ok(format!(
        "v1.{}.{}",
        BASE64.encode(nonce),
        BASE64.encode(ciphertext)
    ))
}

fn decrypt_payload(key: &[u8; 32], envelope: &str) -> Result<Value, String> {
    let mut parts = envelope.split('.');
    if parts.next() != Some("v1") {
        return Err("unsupported encrypted payload version".into());
    }
    let nonce = BASE64
        .decode(parts.next().unwrap_or_default())
        .map_err(|_| "invalid payload nonce".to_string())?;
    let ciphertext = BASE64
        .decode(parts.next().unwrap_or_default())
        .map_err(|_| "invalid encrypted payload".to_string())?;
    if nonce.len() != 24 || parts.next().is_some() {
        return Err("invalid encrypted payload envelope".into());
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| "payload authentication failed".to_string())?;
    serde_json::from_slice(&plaintext).map_err(|_| "decrypted payload is invalid JSON".into())
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        let store = Self(Arc::new(StoreInner {
            connection: Mutex::new(connection),
            encryption_key: key_from_env()?,
        }));
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, String> {
        let mut key = [0_u8; 32];
        rand::thread_rng().fill(&mut key);
        let store = Self(Arc::new(StoreInner {
            connection: Mutex::new(Connection::open_in_memory().map_err(|e| e.to_string())?),
            encryption_key: key,
        }));
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        self.0.connection.lock().unwrap().execute_batch(r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS users (
              id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE COLLATE NOCASE,
              password_hash TEXT NOT NULL, role TEXT NOT NULL CHECK(role IN ('customer','operator')),
              disabled INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
              token_hash TEXT PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
              csrf_token TEXT NOT NULL, expires_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS intakes (
              id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
              form_code TEXT NOT NULL CHECK(form_code IN ('1701Q','1702Q')),
              payload TEXT NOT NULL, revision INTEGER NOT NULL DEFAULT 1,
              state TEXT NOT NULL DEFAULT 'draft' CHECK(state IN ('draft','received')),
              workflow_status TEXT CHECK(workflow_status IN ('Received','Filed','Receipt sent')),
              reference TEXT UNIQUE, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, submitted_at INTEGER
            );
            CREATE INDEX IF NOT EXISTS intakes_user_id ON intakes(user_id);
            CREATE TABLE IF NOT EXISTS login_throttle (
              email TEXT PRIMARY KEY COLLATE NOCASE, failures INTEGER NOT NULL,
              window_started INTEGER NOT NULL, blocked_until INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS deletion_tombstones (
              id INTEGER PRIMARY KEY, deleted_at INTEGER NOT NULL, form_code TEXT NOT NULL,
              prior_workflow_status TEXT, reference_hash TEXT
            );
        "#).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn create_user(&self, email: &str, password: &str, role: &str) -> Result<i64, String> {
        if !matches!(role, "customer" | "operator") {
            return Err("role must be customer or operator".into());
        }
        if password.len() < 12 {
            return Err("password must contain at least 12 characters".into());
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| e.to_string())?
            .to_string();
        let conn = self.0.connection.lock().unwrap();
        conn.execute(
            "INSERT INTO users(email,password_hash,role,created_at) VALUES(?1,?2,?3,?4)",
            params![email.trim().to_ascii_lowercase(), hash, role, now()],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    pub fn list_users(&self) -> Result<Vec<(i64, String, String, bool)>, String> {
        let conn = self.0.connection.lock().unwrap();
        let mut query = conn
            .prepare("SELECT id,email,role,disabled FROM users ORDER BY email")
            .map_err(|e| e.to_string())?;
        let users = query
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, i64>(3)? != 0,
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(users)
    }

    pub fn reset_password(&self, email: &str, password: &str) -> Result<(), String> {
        if password.len() < 12 {
            return Err("password must contain at least 12 characters".into());
        }
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| e.to_string())?
            .to_string();
        let mut conn = self.0.connection.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM users WHERE email=?1 COLLATE NOCASE",
                [email.trim()],
                |row| row.get(0),
            )
            .map_err(|_| "user not found".to_string())?;
        tx.execute(
            "UPDATE users SET password_hash=?1 WHERE id=?2",
            params![hash, id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM sessions WHERE user_id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn set_user_disabled(&self, email: &str, disabled: bool) -> Result<(), String> {
        let mut conn = self.0.connection.lock().unwrap();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let id: i64 = tx
            .query_row(
                "SELECT id FROM users WHERE email=?1 COLLATE NOCASE",
                [email.trim()],
                |row| row.get(0),
            )
            .map_err(|_| "user not found".to_string())?;
        tx.execute(
            "UPDATE users SET disabled=?1 WHERE id=?2",
            params![disabled as i64, id],
        )
        .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM sessions WHERE user_id=?1", [id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())
    }

    fn is_login_throttled(&self, email: &str) -> bool {
        self.0
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT blocked_until FROM login_throttle WHERE email=?1 COLLATE NOCASE",
                [email],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .ok()
            .flatten()
            .unwrap_or_default()
            > now()
    }

    fn record_login_failure(&self, email: &str) -> Result<(), String> {
        let conn = self.0.connection.lock().unwrap();
        let current = conn
            .query_row(
                "SELECT failures,window_started FROM login_throttle WHERE email=?1 COLLATE NOCASE",
                [email],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        let (failures, started) = match current {
            Some((failures, started)) if now() - started <= LOGIN_WINDOW_SECONDS => {
                (failures + 1, started)
            }
            _ => (1, now()),
        };
        let blocked_until = if failures >= LOGIN_MAX_FAILURES {
            now() + LOGIN_BLOCK_SECONDS
        } else {
            0
        };
        conn.execute("INSERT INTO login_throttle(email,failures,window_started,blocked_until) VALUES(?1,?2,?3,?4) ON CONFLICT(email) DO UPDATE SET failures=excluded.failures,window_started=excluded.window_started,blocked_until=excluded.blocked_until", params![email,failures,started,blocked_until]).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn clear_login_failures(&self, email: &str) {
        let _ = self.0.connection.lock().unwrap().execute(
            "DELETE FROM login_throttle WHERE email=?1 COLLATE NOCASE",
            [email],
        );
    }
}

pub fn app(store: Store, static_dir: impl AsRef<Path>) -> Router {
    let state = AppState { store };
    let index = static_dir.as_ref().join("index.html");
    Router::new()
        .route("/api/healthz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/api/operator/intakes/{id}/export", get(operator_export))
        .route("/api/{*fn_name}", post(server_fn_handler))
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)))
        .layer(middleware::from_fn_with_state(state.clone(), load_actor))
        .with_state(state)
}

async fn server_fn_handler(State(state): State<AppState>, request: Request) -> impl IntoResponse {
    leptos_axum::handle_server_fns_with_context(
        move || provide_context(state.store.clone()),
        request,
    )
    .await
}

async fn load_actor(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    if let Some(raw) = cookie(&request.headers().clone(), SESSION_COOKIE) {
        let actor = {
            let conn = state.store.0.connection.lock().unwrap();
            conn.query_row("SELECT users.id,users.email,users.role,sessions.csrf_token FROM sessions JOIN users ON users.id=sessions.user_id WHERE sessions.token_hash=?1 AND sessions.expires_at>?2 AND users.disabled=0",
                params![session_hash(&raw), now()], |r| Ok(Actor{id:r.get(0)?,email:r.get(1)?,role:r.get(2)?,csrf_token:r.get(3)?})).optional().ok().flatten()
        };
        if let Some(actor) = actor {
            request.extensions_mut().insert(actor);
        }
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert("content-security-policy", HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'self'; frame-ancestors 'none'"));
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("DENY"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_string())
        })
}

fn actor(request: &Request) -> ApiResult<Actor> {
    request
        .extensions()
        .get::<Actor>()
        .cloned()
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "authentication required"))
}
fn operator(request: &Request) -> ApiResult<Actor> {
    let actor = actor(request)?;
    if actor.role != "operator" {
        return Err(error(StatusCode::FORBIDDEN, "operator access required"));
    }
    Ok(actor)
}
#[allow(dead_code)]
fn csrf(request: &Request, actor: &Actor) -> ApiResult<()> {
    if request
        .headers()
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        != Some(actor.csrf_token.as_str())
    {
        return Err(error(StatusCode::FORBIDDEN, "invalid CSRF token"));
    }
    Ok(())
}
#[allow(dead_code)]
fn extension_actor(actor: Option<Extension<Actor>>) -> ApiResult<Actor> {
    actor
        .map(|Extension(actor)| actor)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "authentication required"))
}
#[allow(dead_code)]
fn extension_operator(actor: Option<Extension<Actor>>) -> ApiResult<Actor> {
    let actor = extension_actor(actor)?;
    if actor.role != "operator" {
        return Err(error(StatusCode::FORBIDDEN, "operator access required"));
    }
    Ok(actor)
}
#[allow(dead_code)]
fn csrf_headers(headers: &HeaderMap, actor: &Actor) -> ApiResult<()> {
    if headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(actor.csrf_token.as_str())
    {
        return Err(error(StatusCode::FORBIDDEN, "invalid CSRF token"));
    }
    Ok(())
}
fn form_code_for_intake(store: &Store, id: i64, user_id: i64) -> ApiResult<String> {
    let record: Option<(String, String)> = store
        .0
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT form_code,payload FROM intakes WHERE id=?1 AND user_id=?2 AND state='draft'",
            params![id, user_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let (code, encrypted) = record.ok_or_else(|| {
        error(
            StatusCode::CONFLICT,
            "draft is unavailable or already submitted",
        )
    })?;
    decrypt_payload(&store.0.encryption_key, &encrypted).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored payload failed authentication; refusing overwrite",
        )
    })?;
    Ok(code)
}

/* Portal operations below are Leptos server functions. Axum-specific endpoints are
 * intentionally limited to health and authenticated export. */
#[cfg(any())]
async fn removed_legacy_login(
    State(state): State<AppState>,
    Json(input): Json<Value>,
) -> ApiResult<Response> {
    let normalized_email = input.email.trim().to_ascii_lowercase();
    if state.store.is_login_throttled(&normalized_email) {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "too many failed sign-in attempts; try again later",
        ));
    }
    let user = {
        let conn = state.store.0.connection.lock().unwrap();
        conn.query_row("SELECT id,email,password_hash,role FROM users WHERE email=?1 COLLATE NOCASE AND disabled=0", [&normalized_email], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?
    };
    let Some((id, email, hash, role)) = user else {
        state
            .store
            .record_login_failure(&normalized_email)
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
        return Err(error(StatusCode::UNAUTHORIZED, "invalid email or password"));
    };
    let parsed = PasswordHash::new(&hash)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "invalid password record"))?;
    if Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed)
        .is_err()
    {
        state
            .store
            .record_login_failure(&normalized_email)
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
        return Err(error(StatusCode::UNAUTHORIZED, "invalid email or password"));
    }
    state.store.clear_login_failures(&normalized_email);
    let raw = token();
    let csrf_token = token();
    state
        .store
        .0
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO sessions(token_hash,user_id,csrf_token,expires_at) VALUES(?1,?2,?3,?4)",
            params![session_hash(&raw), id, csrf_token, now() + SESSION_SECONDS],
        )
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let secure = std::env::var("EBIRFORMS_WEB_INSECURE_COOKIE")
        .ok()
        .as_deref()
        != Some("1");
    let cookie = format!(
        "{SESSION_COOKIE}={raw}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_SECONDS}{}",
        if secure { "; Secure" } else { "" }
    );
    let mut response =
        Json(json!({"id":id,"email":email,"role":role,"csrf_token":csrf_token})).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap());
    Ok(response)
}
async fn logout(State(state): State<AppState>, request: Request) -> ApiResult<Response> {
    let current = actor(&request)?;
    csrf(&request, &current)?;
    if let Some(raw) = cookie(request.headers(), SESSION_COOKIE) {
        let _ = state.store.0.connection.lock().unwrap().execute(
            "DELETE FROM sessions WHERE token_hash=?1",
            [session_hash(&raw)],
        );
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "ebirforms_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    Ok(response)
}
async fn me(request: Request) -> ApiResult<Json<Value>> {
    let a = actor(&request)?;
    Ok(Json(
        json!({"id":a.id,"email":a.email,"role":a.role,"csrf_token":a.csrf_token}),
    ))
}

fn row_intake(r: &rusqlite::Row<'_>, key: &[u8; 32]) -> rusqlite::Result<Intake> {
    let raw: String = r.get(4)?;
    let payload = decrypt_payload(key, &raw).map_err(|message| {
        rusqlite::Error::FromSqlConversionFailure(
            4,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message,
            )),
        )
    })?;
    Ok(Intake {
        id: r.get(0)?,
        user_id: r.get(1)?,
        owner_email: r.get(2)?,
        form_code: r.get(3)?,
        payload,
        revision: r.get(5)?,
        state: r.get(6)?,
        workflow_status: r.get(7)?,
        reference: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
        submitted_at: r.get(11)?,
    })
}
const INTAKE_SELECT:&str="SELECT intakes.id,intakes.user_id,users.email,intakes.form_code,intakes.payload,intakes.revision,intakes.state,intakes.workflow_status,intakes.reference,intakes.created_at,intakes.updated_at,intakes.submitted_at FROM intakes JOIN users ON users.id=intakes.user_id";

async fn list_my_intakes(
    State(s): State<AppState>,
    request: Request,
) -> ApiResult<Json<Vec<Intake>>> {
    let a = actor(&request)?;
    let c = s.store.0.connection.lock().unwrap();
    let key = s.store.0.encryption_key;
    let mut q = c
        .prepare(&format!(
            "{INTAKE_SELECT} WHERE intakes.user_id=?1 ORDER BY intakes.updated_at DESC"
        ))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let rows = q
        .query_map([a.id], |row| row_intake(row, &key))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "stored payload could not be decrypted",
            )
        })?;
    Ok(Json(rows))
}
#[derive(Deserialize)]
struct NewIntake {
    form_code: String,
}
async fn create_intake(
    State(s): State<AppState>,
    actor: Option<Extension<Actor>>,
    headers: HeaderMap,
    Json(i): Json<NewIntake>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let a = extension_actor(actor)?;
    csrf_headers(&headers, &a)?;
    if a.role != "customer" {
        return Err(error(StatusCode::FORBIDDEN, "customer access required"));
    }
    if !matches!(i.form_code.as_str(), "1701Q" | "1702Q") {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "web intake supports only 1701Q and 1702Q",
        ));
    }
    let payload =
        ebirforms_web_schema::blank_payload(&i.form_code, &a.email, a.id).map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "blank form template is invalid",
            )
        })?;
    let encrypted = encrypt_payload(&s.store.0.encryption_key, &payload).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "payload encryption failed",
        )
    })?;
    let c = s.store.0.connection.lock().unwrap();
    c.execute("INSERT INTO intakes(user_id,form_code,payload,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![a.id,i.form_code,encrypted,now()]).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":c.last_insert_rowid(),"revision":1})),
    ))
}
async fn get_intake(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    request: Request,
) -> ApiResult<Json<Intake>> {
    let a = actor(&request)?;
    let c = s.store.0.connection.lock().unwrap();
    let key = s.store.0.encryption_key;
    let item = c
        .query_row(
            &format!("{INTAKE_SELECT} WHERE intakes.id=?1 AND intakes.user_id=?2"),
            params![id, a.id],
            |row| row_intake(row, &key),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "intake not found"))?;
    Ok(Json(item))
}
#[derive(Deserialize)]
struct SaveIntake {
    payload: Value,
    revision: i64,
}
async fn save_intake(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    actor: Option<Extension<Actor>>,
    headers: HeaderMap,
    Json(i): Json<SaveIntake>,
) -> ApiResult<Json<Value>> {
    let a = extension_actor(actor)?;
    csrf_headers(&headers, &a)?;
    let mut payload = i.payload;
    ebirforms_web_schema::normalize(&form_code_for_intake(&s.store, id, a.id)?, &mut payload);
    let encrypted = encrypt_payload(&s.store.0.encryption_key, &payload).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "payload encryption failed",
        )
    })?;
    let changed=s.store.0.connection.lock().unwrap().execute("UPDATE intakes SET payload=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND user_id=?4 AND revision=?5 AND state='draft'",params![encrypted,now(),id,a.id,i.revision]).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?;
    if changed == 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "draft changed elsewhere, was submitted, or does not exist",
        ));
    }
    Ok(Json(json!({"revision":i.revision+1,"updated_at":now()})))
}
async fn submit_intake(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    request: Request,
) -> ApiResult<Json<Value>> {
    let a = actor(&request)?;
    csrf(&request, &a)?;
    let c = s.store.0.connection.lock().unwrap();
    let (code, raw): (String, String) = c
        .query_row(
            "SELECT form_code,payload FROM intakes WHERE id=?1 AND user_id=?2 AND state='draft'",
            params![id, a.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| {
            error(
                StatusCode::CONFLICT,
                "draft is unavailable or already submitted",
            )
        })?;
    let payload = decrypt_payload(&s.store.0.encryption_key, &raw).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored payload could not be decrypted",
        )
    })?;
    if let Err(errors) = ebirforms_web_schema::validate(&code, &payload) {
        return Err(error(StatusCode::UNPROCESSABLE_ENTITY, errors.join(". ")));
    }
    ebirforms_core::render_form(&code, &payload).map_err(|e| {
        error(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("form validation failed: {e}"),
        )
    })?;
    let reference = reference();
    c.execute("UPDATE intakes SET state='received',workflow_status='Received',reference=?1,submitted_at=?2,updated_at=?2 WHERE id=?3",params![reference,now(),id]).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?;
    Ok(Json(
        json!({"reference":reference,"message":"We received your information. Our team will review it and file the return through the official eBIRForms process. Your official receipt will follow after filing."}),
    ))
}

async fn operator_list(
    State(s): State<AppState>,
    request: Request,
) -> ApiResult<Json<Vec<Intake>>> {
    operator(&request)?;
    let c = s.store.0.connection.lock().unwrap();
    let key = s.store.0.encryption_key;
    let mut q = c
        .prepare(&format!(
            "{INTAKE_SELECT} WHERE intakes.state='received' ORDER BY intakes.updated_at DESC"
        ))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let rows = q
        .query_map([], |row| row_intake(row, &key))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "stored payload could not be decrypted",
            )
        })?;
    Ok(Json(rows))
}
async fn operator_get(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    request: Request,
) -> ApiResult<Json<Intake>> {
    operator(&request)?;
    let c = s.store.0.connection.lock().unwrap();
    let key = s.store.0.encryption_key;
    let item = c
        .query_row(
            &format!("{INTAKE_SELECT} WHERE intakes.id=?1"),
            [id],
            |row| row_intake(row, &key),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "intake not found"))?;
    Ok(Json(item))
}
async fn operator_export(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    request: Request,
) -> ApiResult<Response> {
    operator(&request)?;
    let c = s.store.0.connection.lock().unwrap();
    let raw: String = c
        .query_row("SELECT payload FROM intakes WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "intake not found"))?;
    let payload = decrypt_payload(&s.store.0.encryption_key, &raw).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "stored payload could not be decrypted",
        )
    })?;
    let pretty = serde_json::to_string_pretty(&payload).unwrap();
    let mut res = pretty.into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    res.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=ebirforms-intake-{id}.json")).unwrap(),
    );
    Ok(res)
}
#[derive(Deserialize)]
struct NewStatus {
    status: String,
}
async fn operator_status(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    actor: Option<Extension<Actor>>,
    headers: HeaderMap,
    Json(i): Json<NewStatus>,
) -> ApiResult<Json<Value>> {
    let a = extension_operator(actor)?;
    csrf_headers(&headers, &a)?;
    if !matches!(i.status.as_str(), "Received" | "Filed" | "Receipt sent") {
        return Err(error(StatusCode::BAD_REQUEST, "invalid status"));
    }
    let expected = match i.status.as_str() {
        "Received" => "Received",
        "Filed" => "Received",
        "Receipt sent" => "Filed",
        _ => unreachable!(),
    };
    let changed = s.store.0.connection.lock().unwrap().execute(
        "UPDATE intakes SET workflow_status=?1,updated_at=?2 WHERE id=?3 AND state='received' AND workflow_status=?4",
        params![i.status, now(), id, expected],
    ).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    if changed == 0 {
        return Err(error(StatusCode::CONFLICT, "status can only move forward"));
    }
    Ok(Json(json!({"status":i.status})))
}

#[derive(Deserialize)]
struct DeleteConfirmation {
    confirm: bool,
}

async fn operator_delete(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    actor: Option<Extension<Actor>>,
    headers: HeaderMap,
    Json(confirmation): Json<DeleteConfirmation>,
) -> ApiResult<StatusCode> {
    let a = extension_operator(actor)?;
    csrf_headers(&headers, &a)?;
    if !confirmation.confirm {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "explicit deletion confirmation is required",
        ));
    }
    let mut conn = s.store.0.connection.lock().unwrap();
    let tx = conn
        .transaction()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let record: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT form_code,workflow_status,reference FROM intakes WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let Some((form_code, status, reference)) = record else {
        return Err(error(StatusCode::NOT_FOUND, "intake not found"));
    };
    let reference_hash = reference.map(|value| session_hash(&value));
    tx.execute("INSERT INTO deletion_tombstones(deleted_at,form_code,prior_workflow_status,reference_hash) VALUES(?1,?2,?3,?4)", params![now(),form_code,status,reference_hash]).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    tx.execute("DELETE FROM intakes WHERE id=?1", [id])
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    tx.commit()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
struct NewUser {
    email: String,
    password: String,
    role: String,
}
async fn operator_create_user(
    State(s): State<AppState>,
    actor: Option<Extension<Actor>>,
    headers: HeaderMap,
    Json(i): Json<NewUser>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let a = extension_operator(actor)?;
    csrf_headers(&headers, &a)?;
    let id = s
        .store
        .create_user(&i.email, &i.password, &i.role)
        .map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    Ok((
        StatusCode::CREATED,
        Json(json!({"id":id,"email":i.email,"role":i.role})),
    ))
}

fn portal_error(error: PortalError) -> PortalError {
    let status = match &error {
        PortalError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        PortalError::Forbidden(_) => StatusCode::FORBIDDEN,
        PortalError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
        PortalError::Conflict(_) => StatusCode::CONFLICT,
        PortalError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
        PortalError::NotFound(_) => StatusCode::NOT_FOUND,
        PortalError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    expect_context::<leptos_axum::ResponseOptions>().set_status(status);
    error
}

fn internal(message: impl Into<String>) -> PortalError {
    portal_error(PortalError::Internal(message.into()))
}

async fn request_actor() -> Result<(Actor, HeaderMap), PortalError> {
    let headers = leptos_axum::extract::<HeaderMap>()
        .await
        .map_err(|_| internal("request context is unavailable"))?;
    let raw = cookie(&headers, SESSION_COOKIE)
        .ok_or_else(|| portal_error(PortalError::Unauthorized("authentication required".into())))?;
    let store = expect_context::<Store>();
    let actor = store
        .0
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT users.id,users.email,users.role,sessions.csrf_token FROM sessions JOIN users ON users.id=sessions.user_id WHERE sessions.token_hash=?1 AND sessions.expires_at>?2 AND users.disabled=0",
            params![session_hash(&raw), now()],
            |r| Ok(Actor { id: r.get(0)?, email: r.get(1)?, role: r.get(2)?, csrf_token: r.get(3)? }),
        )
        .optional()
        .map_err(|_| internal("database error"))?
        .ok_or_else(|| portal_error(PortalError::Unauthorized("authentication required".into())))?;
    Ok((actor, headers))
}

async fn authorized(csrf_token: &str, role: Option<&str>) -> Result<Actor, PortalError> {
    let (actor, _) = request_actor().await?;
    if let Some(role) = role {
        if actor.role != role {
            return Err(portal_error(PortalError::Forbidden(format!(
                "{role} access required"
            ))));
        }
    }
    if csrf_token != actor.csrf_token {
        return Err(portal_error(PortalError::Forbidden(
            "invalid CSRF token".into(),
        )));
    }
    Ok(actor)
}

pub async fn get_session_impl() -> Result<Session, PortalError> {
    let (actor, _) = request_actor().await?;
    Ok(Session {
        id: actor.id,
        email: actor.email,
        role: actor.role,
        csrf_token: actor.csrf_token,
    })
}

pub async fn login_impl(email: String, password: String) -> Result<Session, PortalError> {
    let store = expect_context::<Store>();
    let normalized_email = email.trim().to_ascii_lowercase();
    if store.is_login_throttled(&normalized_email) {
        return Err(portal_error(PortalError::RateLimited(
            "too many failed sign-in attempts; try again later".into(),
        )));
    }
    let user = store
        .0
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT id,email,password_hash,role FROM users WHERE email=?1 COLLATE NOCASE AND disabled=0",
            [&normalized_email],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        )
        .optional()
        .map_err(|_| internal("database error"))?;
    let Some((id, email, hash, role)) = user else {
        store
            .record_login_failure(&normalized_email)
            .map_err(internal)?;
        return Err(portal_error(PortalError::Unauthorized(
            "invalid email or password".into(),
        )));
    };
    let parsed = PasswordHash::new(&hash).map_err(|_| internal("invalid password record"))?;
    if Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_err()
    {
        store
            .record_login_failure(&normalized_email)
            .map_err(internal)?;
        return Err(portal_error(PortalError::Unauthorized(
            "invalid email or password".into(),
        )));
    }
    store.clear_login_failures(&normalized_email);
    let raw = token();
    let csrf_token = token();
    store
        .0
        .connection
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO sessions(token_hash,user_id,csrf_token,expires_at) VALUES(?1,?2,?3,?4)",
            params![session_hash(&raw), id, csrf_token, now() + SESSION_SECONDS],
        )
        .map_err(|_| internal("database error"))?;
    let secure = std::env::var("EBIRFORMS_WEB_INSECURE_COOKIE")
        .ok()
        .as_deref()
        != Some("1");
    let value = format!(
        "{SESSION_COOKIE}={raw}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_SECONDS}{}",
        if secure { "; Secure" } else { "" }
    );
    expect_context::<leptos_axum::ResponseOptions>().insert_header(
        header::SET_COOKIE,
        HeaderValue::from_str(&value).map_err(|_| internal("could not set session cookie"))?,
    );
    Ok(Session {
        id,
        email,
        role,
        csrf_token,
    })
}

pub async fn logout_impl(csrf_token: String) -> Result<(), PortalError> {
    let actor = authorized(&csrf_token, None).await?;
    let headers = leptos_axum::extract::<HeaderMap>()
        .await
        .map_err(|_| internal("request context is unavailable"))?;
    if let Some(raw) = cookie(&headers, SESSION_COOKIE) {
        expect_context::<Store>()
            .0
            .connection
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM sessions WHERE token_hash=?1 AND user_id=?2",
                params![session_hash(&raw), actor.id],
            )
            .map_err(|_| internal("database error"))?;
    }
    expect_context::<leptos_axum::ResponseOptions>().insert_header(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "ebirforms_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        ),
    );
    Ok(())
}

fn list_for(
    store: &Store,
    where_clause: &str,
    values: &[&dyn rusqlite::ToSql],
) -> Result<Vec<Intake>, PortalError> {
    let conn = store.0.connection.lock().unwrap();
    let key = store.0.encryption_key;
    let mut query = conn
        .prepare(&format!("{INTAKE_SELECT} {where_clause}"))
        .map_err(|_| internal("database error"))?;
    let rows = query
        .query_map(values, |row| row_intake(row, &key))
        .map_err(|_| internal("database error"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| internal("stored payload could not be decrypted"))?;
    Ok(rows)
}

pub async fn list_intakes_impl() -> Result<Vec<Intake>, PortalError> {
    let (actor, _) = request_actor().await?;
    if actor.role != "customer" {
        return Err(portal_error(PortalError::Forbidden(
            "customer access required".into(),
        )));
    }
    list_for(
        &expect_context::<Store>(),
        "WHERE intakes.user_id=?1 ORDER BY intakes.updated_at DESC",
        &[&actor.id],
    )
}

fn intake_by(store: &Store, id: i64, owner: Option<i64>) -> Result<Intake, PortalError> {
    let conn = store.0.connection.lock().unwrap();
    let key = store.0.encryption_key;
    let (sql, values): (String, Vec<&dyn rusqlite::ToSql>) = if let Some(owner) = owner.as_ref() {
        (
            format!("{INTAKE_SELECT} WHERE intakes.id=?1 AND intakes.user_id=?2"),
            vec![&id, owner],
        )
    } else {
        (format!("{INTAKE_SELECT} WHERE intakes.id=?1"), vec![&id])
    };
    conn.query_row(&sql, values.as_slice(), |row| row_intake(row, &key))
        .optional()
        .map_err(|_| internal("stored payload could not be decrypted"))?
        .ok_or_else(|| portal_error(PortalError::NotFound("intake not found".into())))
}

pub async fn create_intake_impl(
    form_code: String,
    csrf_token: String,
) -> Result<Intake, PortalError> {
    let actor = authorized(&csrf_token, Some("customer")).await?;
    if !matches!(form_code.as_str(), "1701Q" | "1702Q") {
        return Err(portal_error(PortalError::Validation(
            "web intake supports only 1701Q and 1702Q".into(),
        )));
    }
    let store = expect_context::<Store>();
    let payload = ebirforms_web_schema::blank_payload(&form_code, &actor.email, actor.id)
        .map_err(|_| internal("blank form template is invalid"))?;
    let encrypted = encrypt_payload(&store.0.encryption_key, &payload)
        .map_err(|_| internal("payload encryption failed"))?;
    let id = {
        let conn = store.0.connection.lock().unwrap();
        conn.execute(
            "INSERT INTO intakes(user_id,form_code,payload,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![actor.id, form_code, encrypted, now()],
        )
        .map_err(|_| internal("database error"))?;
        conn.last_insert_rowid()
    };
    intake_by(&store, id, Some(actor.id))
}

pub async fn get_intake_impl(id: i64) -> Result<Intake, PortalError> {
    let (actor, _) = request_actor().await?;
    if actor.role != "customer" {
        return Err(portal_error(PortalError::Forbidden(
            "customer access required".into(),
        )));
    }
    intake_by(&expect_context::<Store>(), id, Some(actor.id))
}

pub async fn save_intake_impl(
    id: i64,
    mut payload: Value,
    revision: i64,
    csrf_token: String,
) -> Result<SaveResult, PortalError> {
    let actor = authorized(&csrf_token, Some("customer")).await?;
    let store = expect_context::<Store>();
    let code = form_code_for_intake(&store, id, actor.id)
        .map_err(|(_, body)| portal_error(PortalError::Conflict(body.0.error)))?;
    ebirforms_web_schema::normalize(&code, &mut payload);
    let encrypted = encrypt_payload(&store.0.encryption_key, &payload)
        .map_err(|_| internal("payload encryption failed"))?;
    let updated_at = now();
    let changed = store.0.connection.lock().unwrap().execute(
        "UPDATE intakes SET payload=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND user_id=?4 AND revision=?5 AND state='draft'",
        params![encrypted, updated_at, id, actor.id, revision],
    ).map_err(|_| internal("database error"))?;
    if changed == 0 {
        return Err(portal_error(PortalError::Conflict(
            "draft changed elsewhere, was submitted, or does not exist".into(),
        )));
    }
    Ok(SaveResult {
        revision: revision + 1,
        updated_at,
    })
}

pub async fn submit_intake_impl(
    id: i64,
    csrf_token: String,
) -> Result<SubmissionResult, PortalError> {
    let actor = authorized(&csrf_token, Some("customer")).await?;
    let store = expect_context::<Store>();
    let (code, raw): (String, String) = store
        .0
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT form_code,payload FROM intakes WHERE id=?1 AND user_id=?2 AND state='draft'",
            params![id, actor.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| internal("database error"))?
        .ok_or_else(|| {
            portal_error(PortalError::Conflict(
                "draft is unavailable or already submitted".into(),
            ))
        })?;
    let payload = decrypt_payload(&store.0.encryption_key, &raw)
        .map_err(|_| internal("stored payload could not be decrypted"))?;
    if let Err(errors) = ebirforms_web_schema::validate(&code, &payload) {
        return Err(portal_error(PortalError::Validation(errors.join(". "))));
    }
    ebirforms_core::render_form(&code, &payload).map_err(|error| {
        portal_error(PortalError::Validation(format!(
            "form validation failed: {error}"
        )))
    })?;
    let reference = reference();
    store.0.connection.lock().unwrap().execute(
        "UPDATE intakes SET state='received',workflow_status='Received',reference=?1,submitted_at=?2,updated_at=?2 WHERE id=?3 AND user_id=?4 AND state='draft'",
        params![reference, now(), id, actor.id],
    ).map_err(|_| internal("database error"))?;
    Ok(SubmissionResult {
        reference,
        message: "We received your information. Our team will review it and file the return through the official eBIRForms process. Your official receipt will follow after filing.".into(),
    })
}

pub async fn operator_list_intakes_impl() -> Result<Vec<Intake>, PortalError> {
    let (actor, _) = request_actor().await?;
    if actor.role != "operator" {
        return Err(portal_error(PortalError::Forbidden(
            "operator access required".into(),
        )));
    }
    list_for(
        &expect_context::<Store>(),
        "WHERE intakes.state='received' ORDER BY intakes.updated_at DESC",
        &[],
    )
}

pub async fn operator_get_intake_impl(id: i64) -> Result<Intake, PortalError> {
    let (actor, _) = request_actor().await?;
    if actor.role != "operator" {
        return Err(portal_error(PortalError::Forbidden(
            "operator access required".into(),
        )));
    }
    intake_by(&expect_context::<Store>(), id, None)
}

pub async fn operator_create_account_impl(
    email: String,
    password: String,
    role: String,
    csrf_token: String,
) -> Result<i64, PortalError> {
    authorized(&csrf_token, Some("operator")).await?;
    expect_context::<Store>()
        .create_user(&email, &password, &role)
        .map_err(|error| portal_error(PortalError::Validation(error)))
}

pub async fn operator_update_status_impl(
    id: i64,
    status: String,
    csrf_token: String,
) -> Result<String, PortalError> {
    authorized(&csrf_token, Some("operator")).await?;
    if !matches!(status.as_str(), "Received" | "Filed" | "Receipt sent") {
        return Err(portal_error(PortalError::Validation(
            "invalid status".into(),
        )));
    }
    let expected = match status.as_str() {
        "Received" => "Received",
        "Filed" => "Received",
        "Receipt sent" => "Filed",
        _ => unreachable!(),
    };
    let changed = expect_context::<Store>().0.connection.lock().unwrap().execute(
        "UPDATE intakes SET workflow_status=?1,updated_at=?2 WHERE id=?3 AND state='received' AND workflow_status=?4",
        params![status, now(), id, expected],
    ).map_err(|_| internal("database error"))?;
    if changed == 0 {
        return Err(portal_error(PortalError::Conflict(
            "status can only move forward".into(),
        )));
    }
    Ok(status)
}

pub async fn operator_delete_intake_impl(
    id: i64,
    confirm: bool,
    csrf_token: String,
) -> Result<(), PortalError> {
    authorized(&csrf_token, Some("operator")).await?;
    if !confirm {
        return Err(portal_error(PortalError::Validation(
            "explicit deletion confirmation is required".into(),
        )));
    }
    let store = expect_context::<Store>();
    let mut conn = store.0.connection.lock().unwrap();
    let tx = conn.transaction().map_err(|_| internal("database error"))?;
    let record: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT form_code,workflow_status,reference FROM intakes WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|_| internal("database error"))?;
    let Some((form_code, status, reference)) = record else {
        return Err(portal_error(PortalError::NotFound(
            "intake not found".into(),
        )));
    };
    tx.execute(
        "INSERT INTO deletion_tombstones(deleted_at,form_code,prior_workflow_status,reference_hash) VALUES(?1,?2,?3,?4)",
        params![now(), form_code, status, reference.map(|value| session_hash(&value))],
    ).map_err(|_| internal("database error"))?;
    tx.execute("DELETE FROM intakes WHERE id=?1", [id])
        .map_err(|_| internal("database error"))?;
    tx.commit().map_err(|_| internal("database error"))
}

#[cfg(any())]
mod legacy_tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;

    async fn login_as(router: &Router, email: &str) -> (String, String) {
        let response = router
            .clone()
            .oneshot(
                Request::post("/api/auth/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({"email":email,"password":"correct horse battery staple"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        (cookie, json["csrf_token"].as_str().unwrap().to_string())
    }

    async fn json_request(
        router: &Router,
        method: &str,
        path: &str,
        cookie: &str,
        csrf: &str,
        body: Value,
    ) -> Response {
        router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::COOKIE, cookie)
                    .header("x-csrf-token", csrf)
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn authenticated_customer_and_operator_lifecycle_is_isolated() {
        let store = Store::in_memory().unwrap();
        store
            .create_user(
                "one@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        store
            .create_user(
                "two@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        store
            .create_user(
                "ops@example.test",
                "correct horse battery staple",
                "operator",
            )
            .unwrap();
        let router = app(store.clone(), "/tmp/ebirforms-web-test-assets");
        let (one_cookie, one_csrf) = login_as(&router, "one@example.test").await;
        let (two_cookie, _) = login_as(&router, "two@example.test").await;
        let (ops_cookie, ops_csrf) = login_as(&router, "ops@example.test").await;

        let created = json_request(
            &router,
            "POST",
            "/api/intakes",
            &one_cookie,
            &one_csrf,
            json!({"form_code":"1701Q"}),
        )
        .await;
        assert_eq!(created.status(), StatusCode::CREATED);
        let body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
            .as_i64()
            .unwrap();

        let fresh_submit = json_request(
            &router,
            "POST",
            &format!("/api/intakes/{id}/submit"),
            &one_cookie,
            &one_csrf,
            json!({"confirm":true}),
        )
        .await;
        assert_eq!(fresh_submit.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let second = json_request(
            &router,
            "POST",
            "/api/intakes",
            &one_cookie,
            &one_csrf,
            json!({"form_code":"1702Q"}),
        )
        .await;
        let second_body = to_bytes(second.into_body(), usize::MAX).await.unwrap();
        let second_id = serde_json::from_slice::<Value>(&second_body).unwrap()["id"]
            .as_i64()
            .unwrap();
        let fresh_1702_submit = json_request(
            &router,
            "POST",
            &format!("/api/intakes/{second_id}/submit"),
            &one_cookie,
            &one_csrf,
            json!({}),
        )
        .await;
        assert_eq!(fresh_1702_submit.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let encrypted_at_rest: String = store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT payload FROM intakes WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(encrypted_at_rest.starts_with("v1."));
        assert!(!encrypted_at_rest.contains("123-456-789"));

        let forbidden = router
            .clone()
            .oneshot(
                Request::get(format!("/api/intakes/{id}"))
                    .header(header::COOKIE, &two_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::NOT_FOUND);

        let mut guided_1701 =
            ebirforms_web_schema::blank_payload("1701Q", "one@example.test", 1).unwrap();
        ebirforms_web_schema::fill_with_schema_samples("1701Q", &mut guided_1701);
        let saved = json_request(
            &router,
            "PATCH",
            &format!("/api/intakes/{id}"),
            &one_cookie,
            &one_csrf,
            json!({"payload":guided_1701,"revision":1}),
        )
        .await;
        assert_eq!(saved.status(), StatusCode::OK);
        let encrypted_at_rest: String = store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT payload FROM intakes WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(!encrypted_at_rest.contains("JUAN DELA CRUZ"));
        let submitted = json_request(
            &router,
            "POST",
            &format!("/api/intakes/{id}/submit"),
            &one_cookie,
            &one_csrf,
            json!({"confirm":true}),
        )
        .await;
        assert_eq!(submitted.status(), StatusCode::OK);

        let mut guided_1702 =
            ebirforms_web_schema::blank_payload("1702Q", "one@example.test", 1).unwrap();
        ebirforms_web_schema::fill_with_schema_samples("1702Q", &mut guided_1702);
        let saved_1702 = json_request(
            &router,
            "PATCH",
            &format!("/api/intakes/{second_id}"),
            &one_cookie,
            &one_csrf,
            json!({"payload":guided_1702,"revision":1}),
        )
        .await;
        assert_eq!(saved_1702.status(), StatusCode::OK);
        let submitted_1702 = json_request(
            &router,
            "POST",
            &format!("/api/intakes/{second_id}/submit"),
            &one_cookie,
            &one_csrf,
            json!({}),
        )
        .await;
        assert_eq!(submitted_1702.status(), StatusCode::OK);

        let customer_cannot_operate = router
            .clone()
            .oneshot(
                Request::get("/api/operator/intakes")
                    .header(header::COOKIE, &one_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(customer_cannot_operate.status(), StatusCode::FORBIDDEN);

        let export = router
            .clone()
            .oneshot(
                Request::get(format!("/api/operator/intakes/{id}/export"))
                    .header(header::COOKIE, &ops_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(export.status(), StatusCode::OK);
        assert_eq!(export.headers()[header::CONTENT_TYPE], "application/json");

        for status in ["Filed", "Receipt sent"] {
            let response = json_request(
                &router,
                "PATCH",
                &format!("/api/operator/intakes/{id}/status"),
                &ops_cookie,
                &ops_csrf,
                json!({"status":status}),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
        }
        let cannot_move_back = json_request(
            &router,
            "PATCH",
            &format!("/api/operator/intakes/{id}/status"),
            &ops_cookie,
            &ops_csrf,
            json!({"status":"Filed"}),
        )
        .await;
        assert_eq!(cannot_move_back.status(), StatusCode::CONFLICT);

        let deleted = json_request(
            &router,
            "DELETE",
            &format!("/api/operator/intakes/{id}"),
            &ops_cookie,
            &ops_csrf,
            json!({"confirm":true}),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        let tombstone: (String, Option<String>, Option<String>) = store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT form_code,prior_workflow_status,reference_hash FROM deletion_tombstones",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(tombstone.0, "1701Q");
        assert_eq!(tombstone.1.as_deref(), Some("Receipt sent"));
        assert!(tombstone.2.unwrap().len() == 64);
    }

    #[tokio::test]
    async fn login_is_bounded_and_web_has_no_live_submission_surface() {
        let store = Store::in_memory().unwrap();
        store
            .create_user(
                "bounded@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        let inspect_store = store.clone();
        let router = app(store, "/tmp/ebirforms-web-test-assets");
        for attempt in 1..=LOGIN_MAX_FAILURES {
            let response = router
                .clone()
                .oneshot(
                    Request::post("/api/auth/login")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(
                            json!({"email":"bounded@example.test","password":"wrong password"})
                                .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "attempt {attempt}"
            );
        }
        let blocked=router.clone().oneshot(Request::post("/api/auth/login").header(header::CONTENT_TYPE,"application/json").body(Body::from(json!({"email":"bounded@example.test","password":"correct horse battery staple"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(blocked.status(), StatusCode::TOO_MANY_REQUESTS);
        for path in [
            "/api/live",
            "/api/queue",
            "/api/himalaya",
            "/api/receipts",
            "/api/sftp",
        ] {
            let response = router
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "{path} must not exist"
            );
        }
    }

    #[test]
    fn account_admin_commands_revoke_sessions() {
        let store = Store::in_memory().unwrap();
        let id = store
            .create_user(
                "managed@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        store.0.connection.lock().unwrap().execute("INSERT INTO sessions(token_hash,user_id,csrf_token,expires_at) VALUES('token',?1,'csrf',?2)",params![id,now()+1000]).unwrap();
        store
            .reset_password("managed@example.test", "a different secure password")
            .unwrap();
        let sessions: i64 = store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sessions WHERE user_id=?1",
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sessions, 0);
        store
            .set_user_disabled("managed@example.test", true)
            .unwrap();
        let users = store.list_users().unwrap();
        assert_eq!(
            users,
            vec![(id, "managed@example.test".into(), "customer".into(), true)]
        );
        store
            .set_user_disabled("managed@example.test", false)
            .unwrap();
        assert!(!store.list_users().unwrap()[0].3);
    }

    #[tokio::test]
    async fn authenticated_encryption_failure_is_reported_and_never_overwritten() {
        let store = Store::in_memory().unwrap();
        store
            .create_user(
                "crypto@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        let router = app(store.clone(), "/tmp/ebirforms-web-test-assets");
        let (cookie, csrf) = login_as(&router, "crypto@example.test").await;
        let created = json_request(
            &router,
            "POST",
            "/api/intakes",
            &cookie,
            &csrf,
            json!({"form_code":"1701Q"}),
        )
        .await;
        let body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let id = serde_json::from_slice::<Value>(&body).unwrap()["id"]
            .as_i64()
            .unwrap();
        store
            .0
            .connection
            .lock()
            .unwrap()
            .execute(
                "UPDATE intakes SET payload='v1.invalid.invalid' WHERE id=?1",
                [id],
            )
            .unwrap();
        let read = router
            .clone()
            .oneshot(
                Request::get(format!("/api/intakes/{id}"))
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let overwrite = json_request(
            &router,
            "PATCH",
            &format!("/api/intakes/{id}"),
            &cookie,
            &csrf,
            json!({"payload":{},"revision":1}),
        )
        .await;
        assert_eq!(overwrite.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let stored: String = store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row("SELECT payload FROM intakes WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, "v1.invalid.invalid");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use leptos::server_fn::ServerFn;
    use tower::ServiceExt;

    async fn post_json(router: &Router, path: &str, cookie: Option<&str>, body: Value) -> Response {
        let mut request = Request::post(path).header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        router
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn server_functions_own_portal_surface_and_legacy_routes_are_absent() {
        let store = Store::in_memory().unwrap();
        store
            .create_user(
                "customer@example.test",
                "correct horse battery staple",
                "customer",
            )
            .unwrap();
        let inspect_store = store.clone();
        let router = app(store, "/tmp/ebirforms-web-test-assets");

        assert_eq!(
            router
                .clone()
                .oneshot(Request::get("/api/healthz").body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            StatusCode::NO_CONTENT
        );
        for path in [
            "/api/auth/login",
            "/api/intakes",
            "/api/operator/intakes",
            "/api/live",
            "/api/queue",
            "/api/himalaya",
            "/api/sftp",
        ] {
            let response = router
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert!(
                matches!(
                    response.status(),
                    StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
                ),
                "{path} must not be exposed"
            );
        }

        let response = post_json(
            &router,
            <crate::Login as ServerFn>::PATH,
            None,
            json!({
                "email":"customer@example.test", "password":"correct horse battery staple"
            }),
        )
        .await;
        let login_status = response.status();
        let cookie = response
            .headers()
            .get(header::SET_COOKIE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.split(';').next().unwrap().to_string());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            login_status,
            StatusCode::OK,
            "login path {}, body {}",
            <crate::Login as ServerFn>::PATH,
            String::from_utf8_lossy(&body)
        );
        let cookie = cookie.unwrap();
        let session: Session = serde_json::from_slice(&body).unwrap();
        assert_eq!(session.role, "customer");

        let rejected = post_json(
            &router,
            <crate::CreateIntake as ServerFn>::PATH,
            Some(&cookie),
            json!({"form_code":"1701Q", "csrf_token":"wrong-token"}),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::FORBIDDEN);

        let created = post_json(
            &router,
            <crate::CreateIntake as ServerFn>::PATH,
            Some(&cookie),
            json!({
                "form_code":"1701Q", "csrf_token":session.csrf_token
            }),
        )
        .await;
        assert_eq!(created.status(), StatusCode::OK);
        let body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let intake: Intake = serde_json::from_slice(&body).unwrap();
        assert_eq!(intake.form_code, "1701Q");
        assert_eq!(intake.revision, 1);
        let encrypted: String = inspect_store
            .0
            .connection
            .lock()
            .unwrap()
            .query_row(
                "SELECT payload FROM intakes WHERE id=?1",
                [intake.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(encrypted.starts_with("v1."));
        assert!(!encrypted.contains("customer@example.test"));
    }
}
