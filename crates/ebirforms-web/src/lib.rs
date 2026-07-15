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
use rand::{distributions::Alphanumeric, Rng};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower_http::services::{ServeDir, ServeFile};

const SESSION_COOKIE: &str = "ebirforms_session";
const SESSION_SECONDS: i64 = 60 * 60 * 12;
const FORM_1701Q: &str = include_str!("../../../tests/fixtures/1701Q/input.json");
const FORM_1702Q: &str = include_str!("../../../tests/fixtures/1702Q/input.json");

#[derive(Clone)]
pub struct Store(Arc<Mutex<Connection>>);

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

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let connection = Connection::open(path).map_err(|e| e.to_string())?;
        let store = Self(Arc::new(Mutex::new(connection)));
        store.migrate()?;
        Ok(store)
    }

    pub fn in_memory() -> Result<Self, String> {
        let store = Self(Arc::new(Mutex::new(
            Connection::open_in_memory().map_err(|e| e.to_string())?,
        )));
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), String> {
        self.0.lock().unwrap().execute_batch(r#"
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
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO users(email,password_hash,role,created_at) VALUES(?1,?2,?3,?4)",
            params![email.trim().to_ascii_lowercase(), hash, role, now()],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
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
        .route("/operator/users", post(operator_create_user));
    Router::new()
        .nest("/api", api)
        .fallback_service(ServeDir::new(static_dir).not_found_service(ServeFile::new(index)))
        .layer(middleware::from_fn_with_state(state.clone(), load_actor))
        .with_state(state)
}

async fn load_actor(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    if let Some(raw) = cookie(&request.headers().clone(), SESSION_COOKIE) {
        let actor = {
            let conn = state.store.0.lock().unwrap();
            conn.query_row("SELECT users.id,users.email,users.role,sessions.csrf_token FROM sessions JOIN users ON users.id=sessions.user_id WHERE sessions.token_hash=?1 AND sessions.expires_at>?2 AND users.disabled=0",
                params![session_hash(&raw), now()], |r| Ok(Actor{id:r.get(0)?,email:r.get(1)?,role:r.get(2)?,csrf_token:r.get(3)?})).optional().ok().flatten()
        };
        if let Some(actor) = actor {
            request.extensions_mut().insert(actor);
        }
    }
    next.run(request).await
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

#[derive(Deserialize)]
struct Login {
    email: String,
    password: String,
}
async fn login(State(state): State<AppState>, Json(input): Json<Login>) -> ApiResult<Response> {
    let user = {
        let conn = state.store.0.lock().unwrap();
        conn.query_row("SELECT id,email,password_hash,role FROM users WHERE email=?1 COLLATE NOCASE AND disabled=0", [input.email.trim()], |r| Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?
    };
    let Some((id, email, hash, role)) = user else {
        return Err(error(StatusCode::UNAUTHORIZED, "invalid email or password"));
    };
    let parsed = PasswordHash::new(&hash)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "invalid password record"))?;
    if Argon2::default()
        .verify_password(input.password.as_bytes(), &parsed)
        .is_err()
    {
        return Err(error(StatusCode::UNAUTHORIZED, "invalid email or password"));
    }
    let raw = token();
    let csrf_token = token();
    state
        .store
        .0
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
        let _ = state.store.0.lock().unwrap().execute(
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
fn row_intake(r: &rusqlite::Row<'_>) -> rusqlite::Result<Intake> {
    let raw: String = r.get(4)?;
    Ok(Intake {
        id: r.get(0)?,
        user_id: r.get(1)?,
        owner_email: r.get(2)?,
        form_code: r.get(3)?,
        payload: serde_json::from_str(&raw).unwrap_or(Value::Null),
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
    let c = s.store.0.lock().unwrap();
    let mut q = c
        .prepare(&format!(
            "{INTAKE_SELECT} WHERE intakes.user_id=?1 ORDER BY intakes.updated_at DESC"
        ))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let rows = q
        .query_map([a.id], row_intake)
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
    let template = if i.form_code == "1701Q" {
        FORM_1701Q
    } else {
        FORM_1702Q
    };
    let mut payload: Value = serde_json::from_str(template).map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "form template is invalid",
        )
    })?;
    payload["profile"]["email"] = Value::String(a.email.clone());
    payload["profile"]["profile_id"] = Value::String(format!("web-customer-{}", a.id));
    let c = s.store.0.lock().unwrap();
    c.execute("INSERT INTO intakes(user_id,form_code,payload,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",params![a.id,i.form_code,payload.to_string(),now()]).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?;
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
    let c = s.store.0.lock().unwrap();
    let item = c
        .query_row(
            &format!("{INTAKE_SELECT} WHERE intakes.id=?1 AND intakes.user_id=?2"),
            params![id, a.id],
            row_intake,
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
    let changed=s.store.0.lock().unwrap().execute("UPDATE intakes SET payload=?1,revision=revision+1,updated_at=?2 WHERE id=?3 AND user_id=?4 AND revision=?5 AND state='draft'",params![i.payload.to_string(),now(),id,a.id,i.revision]).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"database error"))?;
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
    let c = s.store.0.lock().unwrap();
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
    let payload: Value = serde_json::from_str(&raw)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "payload must be valid JSON"))?;
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
    let c = s.store.0.lock().unwrap();
    let mut q = c
        .prepare(&format!(
            "{INTAKE_SELECT} WHERE intakes.state='received' ORDER BY intakes.updated_at DESC"
        ))
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    let rows = q
        .query_map([], row_intake)
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
    let c = s.store.0.lock().unwrap();
    let item = c
        .query_row(
            &format!("{INTAKE_SELECT} WHERE intakes.id=?1"),
            [id],
            row_intake,
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
    let c = s.store.0.lock().unwrap();
    let raw: String = c
        .query_row("SELECT payload FROM intakes WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "intake not found"))?;
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null))
            .unwrap();
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
    let current: Option<String> = s
        .store
        .0
        .lock()
        .unwrap()
        .query_row(
            "SELECT workflow_status FROM intakes WHERE id=?1 AND state='received'",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?
        .flatten();
    let allowed = matches!(
        (current.as_deref(), i.status.as_str()),
        (Some("Received"), "Received" | "Filed")
            | (Some("Filed"), "Filed" | "Receipt sent")
            | (Some("Receipt sent"), "Receipt sent")
    );
    if !allowed {
        return Err(error(StatusCode::CONFLICT, "status can only move forward"));
    }
    s.store
        .0
        .lock()
        .unwrap()
        .execute(
            "UPDATE intakes SET workflow_status=?1,updated_at=?2 WHERE id=?3",
            params![i.status, now(), id],
        )
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    Ok(Json(json!({"status":i.status})))
}
async fn operator_delete(
    State(s): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    request: Request,
) -> ApiResult<StatusCode> {
    let a = operator(&request)?;
    csrf(&request, &a)?;
    let n = s
        .store
        .0
        .lock()
        .unwrap()
        .execute("DELETE FROM intakes WHERE id=?1", [id])
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "database error"))?;
    if n == 0 {
        return Err(error(StatusCode::NOT_FOUND, "intake not found"));
    }
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
        let router = app(store, "/tmp/ebirforms-web-test-assets");
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
        let submitted = json_request(
            &router,
            "POST",
            &format!("/api/intakes/{id}/submit"),
            &one_cookie,
            &one_csrf,
            json!({}),
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
            json!({}),
        )
        .await;
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    }
}
