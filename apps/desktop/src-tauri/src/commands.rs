use crate::state;
use ebirforms_core::{
    build_submission_package, parse_and_apply_receipt, render_form, run_due_jobs_dry_run,
    AppSettings, JobMode, SubmissionJob, SubmissionManifest, SubmissionRecord, TaxpayerProfile,
    Theme,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub paths: state::DesktopPaths,
    pub settings: AppSettings,
    pub profiles: Vec<TaxpayerProfile>,
    pub jobs: Vec<SubmissionJob>,
    pub submissions: Vec<SafeSubmissionRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackagePreview {
    pub manifest: SubmissionManifest,
    pub payload_path: String,
    pub payload_sha256_short: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SafeSubmissionRecord {
    pub idempotency_key: String,
    pub idempotency_key_short: String,
    pub status: String,
    pub dry_run: bool,
    pub form_code: String,
    #[serde(rename = "period_mmYYYY")]
    pub period_mm_yyyy: String,
    pub profile_id: String,
    pub remote_path: String,
    pub filename: String,
    pub payload_sha256: String,
    pub payload_sha256_short: String,
    pub payload_size: usize,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub receipt_status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProfileInput {
    pub profile_id: String,
    pub tin: String,
    pub email: String,
    pub taxpayer_name: String,
    #[serde(default)]
    pub rdo_code: Option<String>,
    #[serde(default)]
    pub registered_address: Option<String>,
    #[serde(default)]
    pub zip_code: Option<String>,
}

#[tauri::command]
pub fn app_snapshot(app: AppHandle) -> Result<AppSnapshot, String> {
    let store = state::app_state_store(&app)?;
    let app_state = store.load().map_err(|err| err.to_string())?;
    let jobs = state::job_store(&app)?
        .list()
        .map_err(|err| err.to_string())?;
    let submissions = list_submissions(app.clone())?;
    Ok(AppSnapshot {
        paths: state::paths(&app)?,
        settings: app_state.settings,
        profiles: app_state.profiles,
        jobs,
        submissions,
    })
}

#[tauri::command]
pub fn list_profiles(app: AppHandle) -> Result<Vec<TaxpayerProfile>, String> {
    state::app_state_store(&app)?
        .list_profiles()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn create_profile(app: AppHandle, profile: ProfileInput) -> Result<TaxpayerProfile, String> {
    let mut taxpayer = TaxpayerProfile::new(
        profile.profile_id,
        profile.tin,
        profile.email,
        profile.taxpayer_name,
    );
    taxpayer.rdo_code = clean_optional(profile.rdo_code);
    taxpayer.registered_address = clean_optional(profile.registered_address);
    taxpayer.zip_code = clean_optional(profile.zip_code);
    state::app_state_store(&app)?
        .upsert_profile(taxpayer)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_settings(app: AppHandle, theme: String) -> Result<AppSettings, String> {
    let theme = Theme::parse(&theme).map_err(|err| err.to_string())?;
    state::app_state_store(&app)?
        .set_theme(theme)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn lock_init(app: AppHandle, pin: String) -> Result<AppSettings, String> {
    state::app_state_store(&app)?
        .set_master_pin(&pin)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn unlock_check(app: AppHandle, pin: String) -> Result<bool, String> {
    state::app_state_store(&app)?
        .verify_master_pin(&pin)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn render_1601c(input: Value) -> Result<String, String> {
    render_form("1601C", &input).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn package_1601c(app: AppHandle, input: Value) -> Result<PackagePreview, String> {
    let package = build_submission_package("1601C", &input).map_err(|err| err.to_string())?;
    let artifacts = state::artifacts_dir(&app)?;
    let safe_stem = sanitize_filename(&package.manifest.filename);
    let payload_path = artifacts.join(&safe_stem);
    fs::write(&payload_path, &package.payload)
        .map_err(|err| format!("failed to write payload artifact: {err}"))?;
    Ok(PackagePreview {
        payload_sha256_short: short_hash(&package.manifest.payload_sha256),
        manifest: package.manifest,
        payload_path: payload_path.display().to_string(),
    })
}

#[tauri::command]
pub fn queue_1601c_dry_run(app: AppHandle, input: Value) -> Result<SubmissionJob, String> {
    state::job_store(&app)?
        .enqueue("1601C", &input, JobMode::DryRun, 3)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_jobs(app: AppHandle) -> Result<Vec<SubmissionJob>, String> {
    state::job_store(&app)?
        .list()
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn run_queue_dry_run(
    app: AppHandle,
    limit: Option<usize>,
) -> Result<Vec<SubmissionJob>, String> {
    let jobs = state::job_store(&app)?;
    let submissions = state::submission_store(&app)?;
    run_due_jobs_dry_run(&jobs, &submissions, limit.unwrap_or(10)).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_submissions(app: AppHandle) -> Result<Vec<SafeSubmissionRecord>, String> {
    let records = state::submission_store(&app)?
        .load()
        .map_err(|err| err.to_string())?;
    Ok(records
        .into_iter()
        .map(SafeSubmissionRecord::from)
        .collect())
}

#[tauri::command]
pub fn match_receipt(
    app: AppHandle,
    receipt_text: String,
) -> Result<Vec<SafeSubmissionRecord>, String> {
    let store = state::submission_store(&app)?;
    parse_and_apply_receipt(&store, &receipt_text).map_err(|err| err.to_string())?;
    list_submissions(app)
}

impl From<SubmissionRecord> for SafeSubmissionRecord {
    fn from(record: SubmissionRecord) -> Self {
        let receipt_status = record
            .receipt
            .as_ref()
            .map(|receipt| receipt.status_text.clone());
        Self {
            idempotency_key_short: short_hash(&record.idempotency_key),
            payload_sha256_short: short_hash(&record.payload_sha256),
            status: format!("{:?}", record.status),
            idempotency_key: record.idempotency_key,
            dry_run: record.dry_run,
            form_code: record.form_code,
            period_mm_yyyy: record.period_mm_yyyy,
            profile_id: record.profile_id,
            remote_path: record.remote_path,
            filename: record.filename,
            payload_sha256: record.payload_sha256,
            payload_size: record.payload_size,
            created_unix_seconds: record.created_unix_seconds,
            updated_unix_seconds: record.updated_unix_seconds,
            attempts: record.attempts,
            last_error: record.last_error,
            receipt_status,
        }
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn short_hash(value: &str) -> String {
    value.chars().take(12).collect()
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '#') {
                c
            } else {
                '_'
            }
        })
        .collect()
}
