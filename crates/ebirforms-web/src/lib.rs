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
    routing::{get, patch, post},
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

const SESSION_COOKIE: &str = "ebirforms_session";
const SESSION_SECONDS: i64 = 60 * 60 * 12;
const LOGIN_WINDOW_SECONDS: i64 = 15 * 60;
const LOGIN_MAX_FAILURES: i64 = 5;

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

fn blank_payload(form_code: &str, email: &str, user_id: i64) -> Value {
    let mut fields = serde_json::Map::new();
    let keys: &[&str] = if form_code == "1701Q" {
        &[
            "frm1701q:txtYear",
            "frm1701q:DateQuarter_1",
            "frm1701q:DateQuarter_2",
            "frm1701q:DateQuarter_3",
            "frm1701q:txt5TIN1",
            "frm1701q:txt5TIN2",
            "frm1701q:txt5TIN3",
            "frm1701q:txt5BranchCode",
            "frm1701q:txt5RDOCode",
            "frm1701q:txtTaxPayername",
            "frm1701q:txt11Address",
            "frm1701q:txt14zip",
            "frm1701q:txt15Telno",
            "frm1701q:txt13BirthMonth",
            "frm1701q:txt13BirthDay",
            "frm1701q:txt13BirthYear",
            "ui1701q:taxpayer_citizenship",
            "frm1701q:txt19",
            "frm1701q:txt36A",
            "frm1701q:txt38C",
            "frm1701q:txt38E",
            "frm1701q:txt38I",
            "frm1701q:txt38K",
            "ui1701q:txt55A",
            "ui1701q:txt56A",
            "ui1701q:txt58A",
        ]
    } else {
        &[
            "txtYearEnded",
            "optQuarter1",
            "optQuarter2",
            "optQuarter3",
            "txtTIN1",
            "txtTIN2",
            "txtTIN3",
            "txtBranchCode",
            "txtRDOCode",
            "txtTaxpayerName",
            "txtAddress",
            "txtZipCode",
            "txtTelNum",
            "txtEmail",
            "txtATC",
            "sched1_txtSales1",
            "sched1_txtCost2",
            "sched1_txtOtherIncome4",
            "sched1_txtDeductions6",
            "sched1_txtPrevious8",
            "sched4_txtPriorYearCredits1",
            "sched4_txtPreviousPayments2",
            "sched4_txtCwtCurrent5",
        ]
    };
    for key in keys {
        fields.insert((*key).into(), Value::String(String::new()));
    }
    json!({"profile":{"tin":"","email":email,"profile_id":format!("web-customer-{user_id}")},"return":{"period":{"year":0,"quarter":0},"is_amended":false,"amendment_number":0},"fields":fields})
}

fn required_string<'a>(
    payload: &'a Value,
    pointer: &str,
    label: &str,
    errors: &mut Vec<String>,
) -> Option<&'a str> {
    let value = payload
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        errors.push(format!("{label} is required"));
        None
    } else {
        Some(value)
    }
}

fn validate_guided_payload(form_code: &str, payload: &Value) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    required_string(payload, "/profile/tin", "TIN", &mut errors);
    let year = payload
        .pointer("/return/period/year")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let quarter = payload
        .pointer("/return/period/quarter")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    if !(2000..=2100).contains(&year) {
        errors.push("Tax year must be between 2000 and 2100".into());
    }
    if !(1..=3).contains(&quarter) {
        errors.push("Quarter must be first, second, or third".into());
    }
    let (name, address, rdo) = if form_code == "1701Q" {
        (
            "/fields/frm1701q:txtTaxPayername",
            "/fields/frm1701q:txt11Address",
            "/fields/frm1701q:txt5RDOCode",
        )
    } else {
        (
            "/fields/txtTaxpayerName",
            "/fields/txtAddress",
            "/fields/txtRDOCode",
        )
    };
    required_string(payload, name, "Registered taxpayer name", &mut errors);
    required_string(payload, address, "Registered address", &mut errors);
    required_string(payload, rdo, "RDO code", &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn money(payload: &Value, key: &str) -> f64 {
    payload
        .pointer(&format!("/fields/{key}"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace(',', "")
        .parse()
        .unwrap_or(0.0)
}

fn set_field(payload: &mut Value, key: &str, value: impl Into<String>) {
    if let Some(fields) = payload.get_mut("fields").and_then(Value::as_object_mut) {
        fields.insert(key.into(), Value::String(value.into()));
    }
}

fn normalize_payload(form_code: &str, payload: &mut Value) {
    let tin = payload
        .pointer("/profile/tin")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let year = payload
        .pointer("/return/period/year")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let quarter = payload
        .pointer("/return/period/quarter")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let email = payload
        .pointer("/profile/email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let segments = (
        tin.get(0..3).unwrap_or_default(),
        tin.get(3..6).unwrap_or_default(),
        tin.get(6..9).unwrap_or_default(),
        tin.get(9..14).unwrap_or_default(),
    );
    if form_code == "1701Q" {
        set_field(payload, "frm1701q:txtYear", year.to_string());
        set_field(
            payload,
            "frm1701q:DateQuarter_1",
            (quarter == 1).to_string(),
        );
        set_field(
            payload,
            "frm1701q:DateQuarter_2",
            (quarter == 2).to_string(),
        );
        set_field(
            payload,
            "frm1701q:DateQuarter_3",
            (quarter == 3).to_string(),
        );
        set_field(payload, "frm1701q:txt5TIN1", segments.0);
        set_field(payload, "frm1701q:txt5TIN2", segments.1);
        set_field(payload, "frm1701q:txt5TIN3", segments.2);
        set_field(payload, "frm1701q:txt5BranchCode", segments.3);
        set_field(payload, "txtEmail", email);
        let sales = money(payload, "frm1701q:txt36A");
        let deductions = money(payload, "frm1701q:txt38C");
        let osd = money(payload, "frm1701q:txt38E");
        let prior = money(payload, "frm1701q:txt38I");
        let other = money(payload, "frm1701q:txt38K");
        set_field(
            payload,
            "frm1701q:txt38G",
            format!("{:.2}", sales - deductions.max(osd)),
        );
        set_field(
            payload,
            "frm1701q:txt39A",
            format!("{:.2}", sales - deductions.max(osd) + prior + other),
        );
        let credits = ["ui1701q:txt55A", "ui1701q:txt56A", "ui1701q:txt58A"]
            .iter()
            .map(|key| money(payload, key))
            .sum::<f64>();
        set_field(payload, "ui1701q:txt62A", format!("{credits:.2}"));
    } else {
        let month_day = match quarter {
            1 => "03/31",
            2 => "06/30",
            3 => "09/30",
            _ => "",
        };
        set_field(
            payload,
            "txtYearEnded",
            if month_day.is_empty() {
                String::new()
            } else {
                format!("{month_day}/{year}")
            },
        );
        set_field(payload, "txtTIN1", segments.0);
        set_field(payload, "txtTIN2", segments.1);
        set_field(payload, "txtTIN3", segments.2);
        set_field(payload, "txtBranchCode", segments.3);
        set_field(payload, "txtEmail", email);
        set_field(payload, "optQuarter1", (quarter == 1).to_string());
        set_field(payload, "optQuarter2", (quarter == 2).to_string());
        set_field(payload, "optQuarter3", (quarter == 3).to_string());
        let sales = money(payload, "sched1_txtSales1");
        let cost = money(payload, "sched1_txtCost2");
        let other = money(payload, "sched1_txtOtherIncome4");
        let deductions = money(payload, "sched1_txtDeductions6");
        let prior = money(payload, "sched1_txtPrevious8");
        let gross = sales - cost;
        let total = gross + other;
        let taxable = total - deductions;
        set_field(payload, "sched1_txtGross3", format!("{gross:.2}"));
        set_field(payload, "sched1_txtTotalGross5", format!("{total:.2}"));
        set_field(payload, "sched1_txtTaxable7", format!("{taxable:.2}"));
        set_field(
            payload,
            "sched1_txtTotalTaxable9",
            format!("{:.2}", taxable + prior),
        );
    }
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
            now() + LOGIN_WINDOW_SECONDS
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
    let api = Router::new()
        .route("/healthz", get(|| async { StatusCode::NO_CONTENT }))
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/intakes", get(list_my_intakes).post(create_intake))
        .route("/intakes/:id", get(get_intake).patch(save_intake))
        .route("/intakes/:id/submit", post(submit_intake))
        .route("/operator/intakes", get(operator_list))
        .route(
            "/operator/intakes/:id",
            get(operator_get).delete(operator_delete),
        )
        .route("/operator/intakes/:id/export", get(operator_export))
        .route("/operator/intakes/:id/status", patch(operator_status))
        .route("/operator/users", post(operator_create_user))
        .fallback(|| async { error(StatusCode::NOT_FOUND, "web API endpoint not found") });
    Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)))
        .layer(middleware::from_fn_with_state(state.clone(), load_actor))
        .with_state(state)
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

fn extension_actor(actor: Option<Extension<Actor>>) -> ApiResult<Actor> {
    actor
        .map(|Extension(actor)| actor)
        .ok_or_else(|| error(StatusCode::UNAUTHORIZED, "authentication required"))
}

fn extension_operator(actor: Option<Extension<Actor>>) -> ApiResult<Actor> {
    let actor = extension_actor(actor)?;
    if actor.role != "operator" {
        return Err(error(StatusCode::FORBIDDEN, "operator access required"));
    }
    Ok(actor)
}

fn csrf_headers(headers: &HeaderMap, actor: &Actor) -> ApiResult<()> {
    if headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) != Some(actor.csrf_token.as_str())
    {
        return Err(error(StatusCode::FORBIDDEN, "invalid CSRF token"));
    }
    Ok(())
}

fn form_code_for_intake(store: &Store, id: i64, user_id: i64) -> ApiResult<String> {
    store
        .0
        .connection
        .lock()
        .unwrap()
        .query_row(
            "SELECT form_code FROM intakes WHERE id=?1 AND user_id=?2 AND state='draft'",
            params![id, user_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| {
            error(
                StatusCode::CONFLICT,
                "draft is unavailable or already submitted",
            )
        })
}

#[derive(Deserialize)]
struct Login {
    email: String,
    password: String,
}
async fn login(State(state): State<AppState>, Json(input): Json<Login>) -> ApiResult<Response> {
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

#[derive(Serialize)]
struct Intake {
    id: i64,
    user_id: i64,
    owner_email: String,
    form_code: String,
    payload: Value,
    revision: i64,
    state: String,
    workflow_status: Option<String>,
    reference: Option<String>,
    created_at: i64,
    updated_at: i64,
    submitted_at: Option<i64>,
}
fn row_intake(r: &rusqlite::Row<'_>, key: &[u8; 32]) -> rusqlite::Result<Intake> {
    let raw: String = r.get(4)?;
    Ok(Intake {
        id: r.get(0)?,
        user_id: r.get(1)?,
        owner_email: r.get(2)?,
        form_code: r.get(3)?,
        payload: decrypt_payload(key, &raw).unwrap_or(Value::Null),
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
        .filter_map(Result::ok)
        .collect();
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
    let payload = blank_payload(&i.form_code, &a.email, a.id);
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
    normalize_payload(&form_code_for_intake(&s.store, id, a.id)?, &mut payload);
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
    if let Err(errors) = validate_guided_payload(&code, &payload) {
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
        .filter_map(Result::ok)
        .collect();
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

#[cfg(test)]
mod tests {
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

        let fixture: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/1701Q/input.json")).unwrap();
        let saved = json_request(
            &router,
            "PATCH",
            &format!("/api/intakes/{id}"),
            &one_cookie,
            &one_csrf,
            json!({"payload":fixture,"revision":1}),
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
        assert!(!encrypted_at_rest.contains("AUTHORIZED TEST TAXPAYER"));
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
}
