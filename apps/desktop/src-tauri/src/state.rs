use ebirforms_core::{AppStateStore, JobStore, SubmissionStore};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize)]
pub struct DesktopPaths {
    pub data_dir: String,
    pub state_path: String,
    pub jobs_db_path: String,
    pub submissions_path: String,
    pub artifacts_dir: String,
}

pub fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("failed to resolve app data directory: {err}"))?;
    fs::create_dir_all(&path).map_err(|err| format!("failed to create data directory: {err}"))?;
    Ok(path)
}

pub fn paths(app: &AppHandle) -> Result<DesktopPaths, String> {
    let data = data_dir(app)?;
    let artifacts = data.join("artifacts");
    fs::create_dir_all(&artifacts)
        .map_err(|err| format!("failed to create artifacts directory: {err}"))?;
    Ok(DesktopPaths {
        state_path: data.join("app-state.json").display().to_string(),
        jobs_db_path: data.join("jobs.sqlite").display().to_string(),
        submissions_path: data.join("submissions.json").display().to_string(),
        artifacts_dir: artifacts.display().to_string(),
        data_dir: data.display().to_string(),
    })
}

pub fn app_state_store(app: &AppHandle) -> Result<AppStateStore, String> {
    Ok(AppStateStore::new(data_dir(app)?.join("app-state.json")))
}

pub fn submission_store(app: &AppHandle) -> Result<SubmissionStore, String> {
    Ok(SubmissionStore::new(
        data_dir(app)?.join("submissions.json"),
    ))
}

pub fn job_store(app: &AppHandle) -> Result<JobStore, String> {
    JobStore::open(data_dir(app)?.join("jobs.sqlite")).map_err(|err| err.to_string())
}

pub fn artifacts_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let artifacts = data_dir(app)?.join("artifacts");
    fs::create_dir_all(&artifacts)
        .map_err(|err| format!("failed to create artifacts directory: {err}"))?;
    Ok(artifacts)
}
