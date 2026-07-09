use crate::package::build_submission_package;
use crate::submission::{submit_with_store, SubmissionError, SubmissionStore, SubmitMode};
use crate::transport::{
    DryRunTransport, SftpTransport, SubmissionStatus, SubmissionTransport, TransportError,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("job store failed: {0}")]
    Store(String),
    #[error("job {0} not found")]
    NotFound(i64),
    #[error("queued job input is invalid JSON: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Submission(#[from] SubmissionError),
}

impl From<rusqlite::Error> for JobError {
    fn from(err: rusqlite::Error) -> Self {
        JobError::Store(err.to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JobMode {
    DryRun,
    Live,
}

impl JobMode {
    pub fn as_str(self) -> &'static str {
        match self {
            JobMode::DryRun => "dry_run",
            JobMode::Live => "live",
        }
    }

    fn from_str(value: &str) -> Result<Self, JobError> {
        match value {
            "dry_run" => Ok(JobMode::DryRun),
            "live" => Ok(JobMode::Live),
            other => Err(JobError::Store(format!("unknown job mode {other}"))),
        }
    }

    pub fn submit_mode(self) -> SubmitMode {
        match self {
            JobMode::DryRun => SubmitMode::DryRun,
            JobMode::Live => SubmitMode::Live,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionJob {
    pub id: i64,
    pub form_code: String,
    pub input_json: String,
    pub mode: JobMode,
    pub status: SubmissionStatus,
    pub attempts: u32,
    pub max_attempts: u32,
    pub next_attempt_unix_seconds: u64,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
    pub submission_idempotency_key: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JobStore {
    path: PathBuf,
}

impl JobStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, JobError> {
        let store = Self { path: path.into() };
        store.with_connection(|conn| {
            conn.execute_batch(
                "CREATE TABLE IF NOT EXISTS submission_jobs (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    form_code TEXT NOT NULL,
                    input_json TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    status TEXT NOT NULL,
                    attempts INTEGER NOT NULL DEFAULT 0,
                    max_attempts INTEGER NOT NULL DEFAULT 3,
                    next_attempt_unix_seconds INTEGER NOT NULL DEFAULT 0,
                    created_unix_seconds INTEGER NOT NULL,
                    updated_unix_seconds INTEGER NOT NULL,
                    submission_idempotency_key TEXT,
                    last_error TEXT
                );
                CREATE INDEX IF NOT EXISTS idx_submission_jobs_status_next
                    ON submission_jobs(status, next_attempt_unix_seconds);",
            )?;
            Ok(())
        })?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn enqueue(
        &self,
        form_code: &str,
        input: &Value,
        mode: JobMode,
        max_attempts: u32,
    ) -> Result<SubmissionJob, JobError> {
        let input_json =
            serde_json::to_string(input).map_err(|err| JobError::Store(err.to_string()))?;
        let now = unix_now();
        self.with_connection(|conn| {
            conn.execute(
                "INSERT INTO submission_jobs (
                    form_code, input_json, mode, status, attempts, max_attempts,
                    next_attempt_unix_seconds, created_unix_seconds, updated_unix_seconds
                ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, ?8)",
                params![
                    form_code,
                    input_json,
                    mode.as_str(),
                    status_to_str(&SubmissionStatus::Queued),
                    max_attempts,
                    now,
                    now,
                    now
                ],
            )?;
            let id = conn.last_insert_rowid();
            self.get_with_conn(conn, id)
        })
    }

    pub fn get(&self, id: i64) -> Result<SubmissionJob, JobError> {
        self.with_connection(|conn| self.get_with_conn(conn, id))
    }

    pub fn list(&self) -> Result<Vec<SubmissionJob>, JobError> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, form_code, input_json, mode, status, attempts, max_attempts,
                        next_attempt_unix_seconds, created_unix_seconds, updated_unix_seconds,
                        submission_idempotency_key, last_error
                 FROM submission_jobs ORDER BY id",
            )?;
            let rows = stmt.query_map([], row_to_job)?;
            let mut jobs = Vec::new();
            for row in rows {
                jobs.push(row?);
            }
            Ok(jobs)
        })
    }

    pub fn next_runnable(&self) -> Result<Option<SubmissionJob>, JobError> {
        let now = unix_now();
        self.with_connection(|conn| {
            conn.query_row(
                "SELECT id, form_code, input_json, mode, status, attempts, max_attempts,
                        next_attempt_unix_seconds, created_unix_seconds, updated_unix_seconds,
                        submission_idempotency_key, last_error
                 FROM submission_jobs
                 WHERE status = ?1 AND next_attempt_unix_seconds <= ?2
                 ORDER BY id LIMIT 1",
                params![status_to_str(&SubmissionStatus::Queued), now],
                row_to_job,
            )
            .optional()
            .map_err(JobError::from)
        })
    }

    fn get_with_conn(&self, conn: &Connection, id: i64) -> Result<SubmissionJob, JobError> {
        conn.query_row(
            "SELECT id, form_code, input_json, mode, status, attempts, max_attempts,
                    next_attempt_unix_seconds, created_unix_seconds, updated_unix_seconds,
                    submission_idempotency_key, last_error
             FROM submission_jobs WHERE id = ?1",
            params![id],
            row_to_job,
        )
        .optional()?
        .ok_or(JobError::NotFound(id))
    }

    fn update_job(
        &self,
        id: i64,
        status: SubmissionStatus,
        attempts: u32,
        next_attempt_unix_seconds: u64,
        submission_idempotency_key: Option<&str>,
        last_error: Option<&str>,
    ) -> Result<SubmissionJob, JobError> {
        let now = unix_now();
        self.with_connection(|conn| {
            conn.execute(
                "UPDATE submission_jobs
                 SET status = ?2, attempts = ?3, next_attempt_unix_seconds = ?4,
                     updated_unix_seconds = ?5, submission_idempotency_key = ?6,
                     last_error = ?7
                 WHERE id = ?1",
                params![
                    id,
                    status_to_str(&status),
                    attempts,
                    next_attempt_unix_seconds,
                    now,
                    submission_idempotency_key,
                    last_error
                ],
            )?;
            self.get_with_conn(conn, id)
        })
    }

    fn with_connection<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, JobError>,
    ) -> Result<T, JobError> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|err| JobError::Store(err.to_string()))?;
            }
        }
        let conn = Connection::open(&self.path)?;
        f(&conn)
    }
}

pub fn run_next_job<T: SubmissionTransport>(
    job_store: &JobStore,
    submission_store: &SubmissionStore,
    transport: &mut T,
) -> Result<Option<SubmissionJob>, JobError> {
    let Some(job) = job_store.next_runnable()? else {
        return Ok(None);
    };
    let attempts = job.attempts + 1;
    let running = job_store.update_job(
        job.id,
        SubmissionStatus::Running,
        attempts,
        0,
        job.submission_idempotency_key.as_deref(),
        None,
    )?;

    let input: Value = match serde_json::from_str(&running.input_json) {
        Ok(value) => value,
        Err(err) => {
            let failed = job_store.update_job(
                running.id,
                SubmissionStatus::Failed,
                attempts,
                0,
                running.submission_idempotency_key.as_deref(),
                Some(&format!("validation/input JSON error: {err}")),
            )?;
            return Ok(Some(failed));
        }
    };

    let package = match build_submission_package(&running.form_code, &input) {
        Ok(package) => package,
        Err(err) => {
            let failed = job_store.update_job(
                running.id,
                SubmissionStatus::Failed,
                attempts,
                0,
                running.submission_idempotency_key.as_deref(),
                Some(&format!("validation/package error: {err}")),
            )?;
            return Ok(Some(failed));
        }
    };
    let idempotency_key = crate::transport::idempotency_key(&package);

    match submit_with_store(
        &package,
        submission_store,
        transport,
        running.mode.submit_mode(),
    ) {
        Ok(record) => Ok(Some(job_store.update_job(
            running.id,
            record.status,
            attempts,
            0,
            Some(&idempotency_key),
            None,
        )?)),
        Err(SubmissionError::DuplicateRisk { .. }) => Ok(Some(job_store.update_job(
            running.id,
            SubmissionStatus::Uncertain,
            attempts,
            0,
            Some(&idempotency_key),
            Some("duplicate-risk prior submission requires manual review"),
        )?)),
        Err(SubmissionError::Transport(err)) => {
            let (status, next_attempt, error) = retry_decision(&running, attempts, &err);
            Ok(Some(job_store.update_job(
                running.id,
                status,
                attempts,
                next_attempt,
                Some(&idempotency_key),
                Some(&error),
            )?))
        }
        Err(err) => Ok(Some(job_store.update_job(
            running.id,
            SubmissionStatus::Failed,
            attempts,
            0,
            Some(&idempotency_key),
            Some(&err.to_string()),
        )?)),
    }
}

pub fn run_due_jobs_dry_run(
    job_store: &JobStore,
    submission_store: &SubmissionStore,
    limit: usize,
) -> Result<Vec<SubmissionJob>, JobError> {
    let mut completed = Vec::new();
    for _ in 0..limit {
        let mut transport = DryRunTransport::new();
        match run_next_job(job_store, submission_store, &mut transport)? {
            Some(job) => completed.push(job),
            None => break,
        }
    }
    Ok(completed)
}

pub fn run_due_jobs_live(
    job_store: &JobStore,
    submission_store: &SubmissionStore,
    limit: usize,
) -> Result<Vec<SubmissionJob>, JobError> {
    let mut completed = Vec::new();
    for _ in 0..limit {
        let mut transport = SftpTransport::from_env();
        match run_next_job(job_store, submission_store, &mut transport)? {
            Some(job) => completed.push(job),
            None => break,
        }
    }
    Ok(completed)
}

fn retry_decision(
    job: &SubmissionJob,
    attempts: u32,
    err: &TransportError,
) -> (SubmissionStatus, u64, String) {
    if err.is_uncertain() {
        return (SubmissionStatus::Uncertain, 0, err.to_string());
    }
    if attempts < job.max_attempts && is_retryable_transport_error(err) {
        let delay = 60 * 2_u64.pow((attempts - 1).min(5));
        return (
            SubmissionStatus::Queued,
            unix_now() + delay,
            err.to_string(),
        );
    }
    (SubmissionStatus::Failed, 0, err.to_string())
}

fn is_retryable_transport_error(err: &TransportError) -> bool {
    matches!(err, TransportError::Staging(_))
}

fn row_to_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<SubmissionJob> {
    let mode: String = row.get(3)?;
    let status: String = row.get(4)?;
    Ok(SubmissionJob {
        id: row.get(0)?,
        form_code: row.get(1)?,
        input_json: row.get(2)?,
        mode: JobMode::from_str(&mode).map_err(to_sql_err)?,
        status: str_to_status(&status).map_err(to_sql_err)?,
        attempts: row.get(5)?,
        max_attempts: row.get(6)?,
        next_attempt_unix_seconds: row.get(7)?,
        created_unix_seconds: row.get(8)?,
        updated_unix_seconds: row.get(9)?,
        submission_idempotency_key: row.get(10)?,
        last_error: row.get(11)?,
    })
}

fn to_sql_err(err: JobError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
}

fn status_to_str(status: &SubmissionStatus) -> &'static str {
    match status {
        SubmissionStatus::Queued => "queued",
        SubmissionStatus::Running => "running",
        SubmissionStatus::AwaitingReceipt => "awaiting_receipt",
        SubmissionStatus::Confirmed => "confirmed",
        SubmissionStatus::Failed => "failed",
        SubmissionStatus::Uncertain => "uncertain",
        SubmissionStatus::Cancelled => "cancelled",
    }
}

fn str_to_status(value: &str) -> Result<SubmissionStatus, JobError> {
    match value {
        "queued" => Ok(SubmissionStatus::Queued),
        "running" => Ok(SubmissionStatus::Running),
        "awaiting_receipt" => Ok(SubmissionStatus::AwaitingReceipt),
        "confirmed" => Ok(SubmissionStatus::Confirmed),
        "failed" => Ok(SubmissionStatus::Failed),
        "uncertain" => Ok(SubmissionStatus::Uncertain),
        "cancelled" => Ok(SubmissionStatus::Cancelled),
        other => Err(JobError::Store(format!("unknown job status {other}"))),
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{SubmissionTransport, TransportReceipt};
    use std::fs;

    fn fixture_input() -> Value {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C/input.json");
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn temp_path(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("{name}-{}.db", std::process::id()));
        let _ = fs::remove_file(&path);
        path
    }

    #[test]
    fn queued_1601c_job_executes_through_dry_run_transport() {
        let db_path = temp_path("ebirforms-job-dry-run");
        let records_path = temp_path("ebirforms-job-records").with_extension("json");
        let store = JobStore::open(&db_path).unwrap();
        let submission_store = SubmissionStore::new(&records_path);
        let job = store
            .enqueue("1601C", &fixture_input(), JobMode::DryRun, 3)
            .unwrap();

        assert_eq!(job.status, SubmissionStatus::Queued);
        let ran = run_due_jobs_dry_run(&store, &submission_store, 1).unwrap();

        assert_eq!(ran.len(), 1);
        assert_eq!(ran[0].status, SubmissionStatus::AwaitingReceipt);
        assert_eq!(ran[0].attempts, 1);
        assert!(ran[0].submission_idempotency_key.is_some());
        assert_eq!(submission_store.load().unwrap().len(), 1);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&records_path);
    }

    #[test]
    fn invalid_job_input_fails_without_retry() {
        let db_path = temp_path("ebirforms-job-invalid");
        let records_path = temp_path("ebirforms-job-invalid-records").with_extension("json");
        let store = JobStore::open(&db_path).unwrap();
        let submission_store = SubmissionStore::new(&records_path);
        let mut input = fixture_input();
        input["return"]["period"]["month"] = Value::Null;
        store.enqueue("1601C", &input, JobMode::DryRun, 3).unwrap();

        let ran = run_due_jobs_dry_run(&store, &submission_store, 1).unwrap();

        assert_eq!(ran[0].status, SubmissionStatus::Failed);
        assert_eq!(ran[0].attempts, 1);
        assert!(ran[0]
            .last_error
            .as_deref()
            .unwrap_or_default()
            .contains("validation/package error"));
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&records_path);
    }

    struct UncertainTransport;

    impl SubmissionTransport for UncertainTransport {
        fn submit(
            &mut self,
            package: &crate::package::SubmissionPackage,
        ) -> Result<TransportReceipt, TransportError> {
            let _ = package;
            Err(TransportError::UncertainUpload(
                "network ambiguity".to_string(),
            ))
        }
    }

    struct RetryableTransport;

    impl SubmissionTransport for RetryableTransport {
        fn submit(
            &mut self,
            _package: &crate::package::SubmissionPackage,
        ) -> Result<TransportReceipt, TransportError> {
            Err(TransportError::Staging(
                "temporary local staging error".to_string(),
            ))
        }
    }

    #[test]
    fn retryable_network_style_failure_requeues_with_backoff() {
        let db_path = temp_path("ebirforms-job-retry");
        let records_path = temp_path("ebirforms-job-retry-records").with_extension("json");
        let store = JobStore::open(&db_path).unwrap();
        let submission_store = SubmissionStore::new(&records_path);
        store
            .enqueue("1601C", &fixture_input(), JobMode::Live, 3)
            .unwrap();
        let mut transport = RetryableTransport;

        let ran = run_next_job(&store, &submission_store, &mut transport)
            .unwrap()
            .unwrap();

        assert_eq!(ran.status, SubmissionStatus::Queued);
        assert_eq!(ran.attempts, 1);
        assert!(ran.next_attempt_unix_seconds > 0);
        assert!(ran.last_error.unwrap().contains("staging"));
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&records_path);
    }

    #[test]
    fn uncertain_transport_failure_marks_job_uncertain_without_retry() {
        let db_path = temp_path("ebirforms-job-uncertain");
        let records_path = temp_path("ebirforms-job-uncertain-records").with_extension("json");
        let store = JobStore::open(&db_path).unwrap();
        let submission_store = SubmissionStore::new(&records_path);
        store
            .enqueue("1601C", &fixture_input(), JobMode::Live, 3)
            .unwrap();
        let mut transport = UncertainTransport;

        let ran = run_next_job(&store, &submission_store, &mut transport)
            .unwrap()
            .unwrap();

        assert_eq!(ran.status, SubmissionStatus::Uncertain);
        assert_eq!(ran.attempts, 1);
        assert_eq!(ran.next_attempt_unix_seconds, 0);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_file(&records_path);
    }
}
