use crate::package::SubmissionPackage;
use crate::transport::{idempotency_key, SubmissionStatus, SubmissionTransport, TransportError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum SubmissionError {
    #[error("submission with idempotency key `{idempotency_key}` already has status {status:?}; manual review required before retry")]
    DuplicateRisk {
        idempotency_key: String,
        status: SubmissionStatus,
    },
    #[error("submission record store failed: {0}")]
    Store(String),
    #[error(transparent)]
    Transport(#[from] TransportError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmissionRecord {
    pub idempotency_key: String,
    pub status: SubmissionStatus,
    pub dry_run: bool,
    pub form_code: String,
    pub form_version: String,
    #[serde(rename = "period_mmYYYY")]
    pub period_mm_yyyy: String,
    pub profile_id: String,
    pub remote_path: String,
    pub filename: String,
    pub payload_sha256: String,
    pub payload_size: usize,
    pub plaintext_sha256: String,
    pub created_unix_seconds: u64,
    pub updated_unix_seconds: u64,
    pub attempts: u32,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl SubmissionRecord {
    pub fn from_package(
        package: &SubmissionPackage,
        dry_run: bool,
        status: SubmissionStatus,
    ) -> Self {
        let now = unix_now();
        Self {
            idempotency_key: idempotency_key(package),
            status,
            dry_run,
            form_code: package.manifest.form_code.clone(),
            form_version: package.manifest.form_version.clone(),
            period_mm_yyyy: package.manifest.period_mm_yyyy.clone(),
            profile_id: package.manifest.profile_id.clone(),
            remote_path: package.manifest.remote_path.clone(),
            filename: package.manifest.filename.clone(),
            payload_sha256: package.manifest.payload_sha256.clone(),
            payload_size: package.manifest.payload_size,
            plaintext_sha256: package.manifest.plaintext_sha256.clone(),
            created_unix_seconds: now,
            updated_unix_seconds: now,
            attempts: 0,
            last_error: None,
        }
    }

    fn touch(&mut self) {
        self.updated_unix_seconds = unix_now();
    }
}

#[derive(Debug, Clone)]
pub struct SubmissionStore {
    path: PathBuf,
}

impl SubmissionStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Vec<SubmissionRecord>, SubmissionError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = fs::read(&self.path).map_err(|err| SubmissionError::Store(err.to_string()))?;
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_slice(&bytes).map_err(|err| SubmissionError::Store(err.to_string()))
    }

    pub fn save(&self, records: &[SubmissionRecord]) -> Result<(), SubmissionError> {
        if let Some(parent) = self.path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| SubmissionError::Store(err.to_string()))?;
            }
        }
        let bytes = serde_json::to_vec_pretty(records)
            .map_err(|err| SubmissionError::Store(err.to_string()))?;
        fs::write(&self.path, bytes).map_err(|err| SubmissionError::Store(err.to_string()))
    }

    pub fn find(&self, key: &str) -> Result<Option<SubmissionRecord>, SubmissionError> {
        Ok(self
            .load()?
            .into_iter()
            .find(|record| record.idempotency_key == key))
    }

    pub fn upsert(&self, record: SubmissionRecord) -> Result<(), SubmissionError> {
        let mut records = self.load()?;
        match records
            .iter_mut()
            .find(|existing| existing.idempotency_key == record.idempotency_key)
        {
            Some(existing) => *existing = record,
            None => records.push(record),
        }
        self.save(&records)
    }

    pub fn update_status(
        &self,
        key: &str,
        status: SubmissionStatus,
        last_error: Option<String>,
    ) -> Result<SubmissionRecord, SubmissionError> {
        let mut records = self.load()?;
        let record = records
            .iter_mut()
            .find(|record| record.idempotency_key == key)
            .ok_or_else(|| SubmissionError::Store(format!("missing submission record {key}")))?;
        record.status = status;
        record.last_error = last_error;
        record.touch();
        let out = record.clone();
        self.save(&records)?;
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitMode {
    DryRun,
    Live,
}

pub fn submit_with_store<T: SubmissionTransport>(
    package: &SubmissionPackage,
    store: &SubmissionStore,
    transport: &mut T,
    mode: SubmitMode,
) -> Result<SubmissionRecord, SubmissionError> {
    let key = idempotency_key(package);
    let dry_run = mode == SubmitMode::DryRun;
    if let Some(existing) = store.find(&key)? {
        if !existing.dry_run && blocks_automatic_retry(&existing.status) {
            return Err(SubmissionError::DuplicateRisk {
                idempotency_key: key,
                status: existing.status,
            });
        }
    }

    let mut record = SubmissionRecord::from_package(package, dry_run, SubmissionStatus::Running);
    record.attempts = 1;
    store.upsert(record.clone())?;

    match transport.submit(package) {
        Ok(receipt) => store.update_status(&key, receipt.status, None),
        Err(err) => {
            let status = if err.is_uncertain() {
                SubmissionStatus::Uncertain
            } else {
                SubmissionStatus::Failed
            };
            let _ = store.update_status(&key, status, Some(err.to_string()))?;
            Err(SubmissionError::Transport(err))
        }
    }
}

pub fn blocks_automatic_retry(status: &SubmissionStatus) -> bool {
    matches!(
        status,
        SubmissionStatus::Running
            | SubmissionStatus::AwaitingReceipt
            | SubmissionStatus::Confirmed
            | SubmissionStatus::Uncertain
    )
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
    use crate::package::build_submission_package;
    use crate::transport::{DryRunTransport, SftpTransport};
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_input() -> Value {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C/input.json");
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn dry_run_persists_submission_record_before_transport_result() {
        let path = std::env::temp_dir().join(format!(
            "ebirforms-submissions-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let store = SubmissionStore::new(&path);
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut transport = DryRunTransport::new();

        let record =
            submit_with_store(&package, &store, &mut transport, SubmitMode::DryRun).unwrap();

        assert_eq!(record.status, SubmissionStatus::AwaitingReceipt);
        assert!(record.dry_run);
        assert_eq!(record.payload_sha256, package.manifest.payload_sha256);
        assert_eq!(store.load().unwrap().len(), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn dry_run_records_do_not_block_repeat_dry_runs() {
        let path = std::env::temp_dir().join(format!(
            "ebirforms-dry-repeat-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let store = SubmissionStore::new(&path);
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut first_transport = DryRunTransport::new();
        let mut second_transport = DryRunTransport::new();

        submit_with_store(&package, &store, &mut first_transport, SubmitMode::DryRun).unwrap();
        let second =
            submit_with_store(&package, &store, &mut second_transport, SubmitMode::DryRun).unwrap();

        assert_eq!(second.status, SubmissionStatus::AwaitingReceipt);
        assert_eq!(store.load().unwrap().len(), 1);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn uncertain_prior_record_blocks_automatic_retry() {
        let path = std::env::temp_dir().join(format!(
            "ebirforms-uncertain-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let store = SubmissionStore::new(&path);
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let record = SubmissionRecord::from_package(&package, false, SubmissionStatus::Uncertain);
        store.upsert(record).unwrap();
        let mut transport = DryRunTransport::new();

        let err = submit_with_store(&package, &store, &mut transport, SubmitMode::Live)
            .expect_err("uncertain prior upload should block retry");

        assert!(matches!(err, SubmissionError::DuplicateRisk { .. }));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn live_configuration_failure_is_persisted_as_failed() {
        let path = std::env::temp_dir().join(format!(
            "ebirforms-live-failed-test-{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let store = SubmissionStore::new(&path);
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut transport = SftpTransport::unconfigured();

        let err = submit_with_store(&package, &store, &mut transport, SubmitMode::Live)
            .expect_err("missing live config should fail safely");

        assert!(matches!(err, SubmissionError::Transport(_)));
        let records = store.load().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, SubmissionStatus::Failed);
        assert!(records[0].last_error.is_some());
        let _ = fs::remove_file(&path);
    }
}
