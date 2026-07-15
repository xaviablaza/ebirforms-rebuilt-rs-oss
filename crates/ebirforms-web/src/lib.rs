use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "ssr")]
mod server;
#[cfg(feature = "ssr")]
pub use server::{app, Store};

#[cfg(feature = "csr")]
mod ui;
#[cfg(feature = "csr")]
pub use ui::mount_app;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum PortalError {
    Unauthorized(String),
    Forbidden(String),
    Validation(String),
    Conflict(String),
    RateLimited(String),
    NotFound(String),
    Internal(String),
}

impl std::fmt::Display for PortalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&serde_json::to_string(self).map_err(|_| std::fmt::Error)?)
    }
}

impl PortalError {
    pub fn message(&self) -> &str {
        match self {
            Self::Unauthorized(v)
            | Self::Forbidden(v)
            | Self::Validation(v)
            | Self::Conflict(v)
            | Self::RateLimited(v)
            | Self::NotFound(v)
            | Self::Internal(v) => v,
        }
    }
}

impl std::error::Error for PortalError {}

impl std::str::FromStr for PortalError {
    type Err = serde_json::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        serde_json::from_str(value)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Session {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub csrf_token: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Intake {
    pub id: i64,
    pub user_id: i64,
    pub owner_email: String,
    pub form_code: String,
    pub payload: Value,
    pub revision: i64,
    pub state: String,
    pub workflow_status: Option<String>,
    pub reference: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub submitted_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SaveResult {
    pub revision: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SubmissionResult {
    pub reference: String,
    pub message: String,
}

type PortalResult<T> = Result<T, leptos::server_fn::ServerFnError<PortalError>>;

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn get_session() -> PortalResult<Session> {
    server::get_session_impl().await.map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn login(email: String, password: String) -> PortalResult<Session> {
    server::login_impl(email, password)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn logout(csrf_token: String) -> PortalResult<()> {
    server::logout_impl(csrf_token).await.map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn list_intakes() -> PortalResult<Vec<Intake>> {
    server::list_intakes_impl().await.map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn create_intake(form_code: String, csrf_token: String) -> PortalResult<Intake> {
    server::create_intake_impl(form_code, csrf_token)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn get_intake(id: i64) -> PortalResult<Intake> {
    server::get_intake_impl(id).await.map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn save_intake(
    id: i64,
    payload: Value,
    revision: i64,
    csrf_token: String,
) -> PortalResult<SaveResult> {
    server::save_intake_impl(id, payload, revision, csrf_token)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn submit_intake(id: i64, csrf_token: String) -> PortalResult<SubmissionResult> {
    server::submit_intake_impl(id, csrf_token)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn operator_list_intakes() -> PortalResult<Vec<Intake>> {
    server::operator_list_intakes_impl()
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn operator_get_intake(id: i64) -> PortalResult<Intake> {
    server::operator_get_intake_impl(id)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn operator_create_account(
    email: String,
    password: String,
    role: String,
    csrf_token: String,
) -> PortalResult<i64> {
    server::operator_create_account_impl(email, password, role, csrf_token)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn operator_update_status(
    id: i64,
    status: String,
    csrf_token: String,
) -> PortalResult<String> {
    server::operator_update_status_impl(id, status, csrf_token)
        .await
        .map_err(Into::into)
}

#[server(prefix = "/api", input = leptos::server_fn::codec::Json, output = leptos::server_fn::codec::Json)]
pub async fn operator_delete_intake(
    id: i64,
    confirm: bool,
    csrf_token: String,
) -> PortalResult<()> {
    server::operator_delete_intake_impl(id, confirm, csrf_token)
        .await
        .map_err(Into::into)
}
