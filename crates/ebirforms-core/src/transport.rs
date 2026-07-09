use crate::package::SubmissionPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("live SFTP transport requires --live --confirm and configured credentials")]
    LiveNotConfigured,
    #[error("submission with idempotency key `{0}` already exists; refusing duplicate upload")]
    DuplicateRisk(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SubmissionStatus {
    Queued,
    Running,
    AwaitingReceipt,
    Confirmed,
    Failed,
    Uncertain,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransportReceipt {
    pub dry_run: bool,
    pub status: SubmissionStatus,
    pub remote_path: String,
    pub filename: String,
    pub payload_size: usize,
    pub payload_sha256: String,
    pub idempotency_key: String,
}

pub trait SubmissionTransport {
    fn submit(&mut self, package: &SubmissionPackage) -> Result<TransportReceipt, TransportError>;
}

#[derive(Debug, Default)]
pub struct DryRunTransport {
    seen_idempotency_keys: BTreeSet<String>,
}

impl DryRunTransport {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SubmissionTransport for DryRunTransport {
    fn submit(&mut self, package: &SubmissionPackage) -> Result<TransportReceipt, TransportError> {
        let key = idempotency_key(package);
        if !self.seen_idempotency_keys.insert(key.clone()) {
            return Err(TransportError::DuplicateRisk(key));
        }

        Ok(TransportReceipt {
            dry_run: true,
            status: SubmissionStatus::AwaitingReceipt,
            remote_path: package.manifest.remote_path.clone(),
            filename: package.manifest.filename.clone(),
            payload_size: package.manifest.payload_size,
            payload_sha256: package.manifest.payload_sha256.clone(),
            idempotency_key: key,
        })
    }
}

#[derive(Debug, Default)]
pub struct SftpTransport;

impl SubmissionTransport for SftpTransport {
    fn submit(&mut self, _package: &SubmissionPackage) -> Result<TransportReceipt, TransportError> {
        // The safe gate is intentionally wired before any network implementation.
        // A later milestone can add credentials and SSH/SFTP upload here while
        // preserving this trait and the CLI's --live --confirm requirement.
        Err(TransportError::LiveNotConfigured)
    }
}

pub fn idempotency_key(package: &SubmissionPackage) -> String {
    format!(
        "{}:{}:{}",
        package.manifest.form_code,
        package.manifest.period_mm_yyyy,
        package.manifest.payload_sha256
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::build_submission_package;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_input() -> Value {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/1601C/input.json");
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn dry_run_reports_remote_path_size_hash_without_network() {
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut transport = DryRunTransport::new();

        let receipt = transport.submit(&package).unwrap();

        assert!(receipt.dry_run);
        assert_eq!(receipt.remote_path, package.manifest.remote_path);
        assert_eq!(receipt.payload_size, package.payload.len());
        assert_eq!(receipt.payload_sha256, package.manifest.payload_sha256);
    }

    #[test]
    fn dry_run_blocks_duplicate_idempotency_key() {
        let package = build_submission_package("1601C", &fixture_input()).unwrap();
        let mut transport = DryRunTransport::new();

        transport.submit(&package).unwrap();
        let err = transport
            .submit(&package)
            .expect_err("duplicate should block");
        assert!(matches!(err, TransportError::DuplicateRisk(_)));
    }
}
