use crate::package::SubmissionPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("live SFTP transport requires configured BIR_SFTP_* environment variables")]
    MissingLiveConfig,
    #[error("submission with idempotency key `{0}` already exists; refusing duplicate upload")]
    DuplicateRisk(String),
    #[error("SFTP upload failed before a definitive server acknowledgement: {0}")]
    UncertainUpload(String),
    #[error("failed to stage SFTP upload file: {0}")]
    Staging(String),
}

impl TransportError {
    pub fn is_uncertain(&self) -> bool {
        matches!(self, TransportError::UncertainUpload(_))
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key: Option<PathBuf>,
    pub known_hosts: Option<PathBuf>,
}

impl SftpConfig {
    pub fn from_env() -> Option<Self> {
        let host = std::env::var("BIR_SFTP_HOST").ok()?;
        let username = std::env::var("BIR_SFTP_USERNAME").ok()?;
        let port = std::env::var("BIR_SFTP_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(23);
        let password = std::env::var("BIR_SFTP_PASSWORD").ok();
        let private_key = std::env::var("BIR_SFTP_PRIVATE_KEY")
            .ok()
            .map(PathBuf::from);
        let known_hosts = std::env::var("BIR_SFTP_KNOWN_HOSTS")
            .ok()
            .map(PathBuf::from);
        Some(Self {
            host,
            port,
            username,
            password,
            private_key,
            known_hosts,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SftpTransport {
    config: Option<SftpConfig>,
}

impl SftpTransport {
    pub fn from_env() -> Self {
        Self {
            config: SftpConfig::from_env(),
        }
    }

    pub fn unconfigured() -> Self {
        Self { config: None }
    }

    pub fn with_config(config: SftpConfig) -> Self {
        Self {
            config: Some(config),
        }
    }
}

impl Default for SftpTransport {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SubmissionTransport for SftpTransport {
    fn submit(&mut self, package: &SubmissionPackage) -> Result<TransportReceipt, TransportError> {
        let config = self
            .config
            .as_ref()
            .ok_or(TransportError::MissingLiveConfig)?;
        let staged_path =
            std::env::temp_dir().join(format!("ebirforms-{}", package.manifest.filename));
        std::fs::write(&staged_path, &package.payload)
            .map_err(|err| TransportError::Staging(err.to_string()))?;

        // Use the system sftp client so secrets never enter Cargo dependencies or logs.
        // Batch mode keeps this non-interactive; password auth should be supplied by an
        // external ssh-agent/askpass wrapper or future credential module, not printed here.
        let remote_target = format!(
            "{}@{}:{}",
            config.username, config.host, package.manifest.remote_directory
        );
        let mut command = Command::new("sftp");
        command
            .arg("-P")
            .arg(config.port.to_string())
            .arg("-b")
            .arg("-");
        if let Some(key) = &config.private_key {
            command.arg("-i").arg(key);
        }
        if let Some(known_hosts) = &config.known_hosts {
            command
                .arg("-o")
                .arg(format!("UserKnownHostsFile={}", known_hosts.display()));
        }
        command.arg(remote_target);
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            writeln!(
                stdin,
                "put {} {}",
                staged_path.display(),
                package.manifest.filename
            )
            .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;

        let _ = std::fs::remove_file(&staged_path);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TransportError::UncertainUpload(redact_transport_error(
                &stderr,
            )));
        }

        Ok(TransportReceipt {
            dry_run: false,
            status: SubmissionStatus::AwaitingReceipt,
            remote_path: package.manifest.remote_path.clone(),
            filename: package.manifest.filename.clone(),
            payload_size: package.manifest.payload_size,
            payload_sha256: package.manifest.payload_sha256.clone(),
            idempotency_key: idempotency_key(package),
        })
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

fn redact_transport_error(stderr: &str) -> String {
    stderr
        .lines()
        .take(3)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
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
