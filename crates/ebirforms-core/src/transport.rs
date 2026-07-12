use crate::package::SubmissionPackage;
use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::collections::BTreeSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("live SFTP transport requires configured FILING_SFTP_* environment variables")]
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
        Self::from_runtime_env().or_else(Self::from_build_time_defaults)
    }

    fn from_runtime_env() -> Option<Self> {
        let host = std::env::var("FILING_SFTP_HOST").ok()?;
        let username = std::env::var("FILING_SFTP_USERNAME").ok()?;
        let port = std::env::var("FILING_SFTP_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(23);
        let password = std::env::var("FILING_SFTP_PASSWORD").ok();
        let private_key = std::env::var("FILING_SFTP_PRIVATE_KEY")
            .ok()
            .map(PathBuf::from);
        let known_hosts = std::env::var("FILING_SFTP_KNOWN_HOSTS")
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

    fn from_build_time_defaults() -> Option<Self> {
        let host = option_env!("BIR_PRODUCTION_SFTP_HOST")?;
        let username = option_env!("BIR_PRODUCTION_SFTP_USERNAME")?;
        let password = option_env!("BIR_PRODUCTION_SFTP_PASSWORD");
        let private_key = option_env!("BIR_PRODUCTION_SFTP_PRIVATE_KEY").map(PathBuf::from);
        if password.is_none() && private_key.is_none() {
            return None;
        }
        Some(Self {
            host: host.to_string(),
            port: option_env!("BIR_PRODUCTION_SFTP_PORT")
                .and_then(|value| value.parse().ok())
                .unwrap_or(23),
            username: username.to_string(),
            password: password.map(ToOwned::to_owned),
            private_key,
            known_hosts: option_env!("BIR_PRODUCTION_SFTP_KNOWN_HOSTS").map(PathBuf::from),
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
        upload_via_ssh2(config, package)?;

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

fn upload_via_ssh2(config: &SftpConfig, package: &SubmissionPackage) -> Result<(), TransportError> {
    let tcp = TcpStream::connect((config.host.as_str(), config.port))
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    let mut session =
        Session::new().map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;

    if let Some(password) = &config.password {
        session
            .userauth_password(&config.username, password)
            .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    } else if let Some(private_key) = &config.private_key {
        session
            .userauth_pubkey_file(&config.username, None, private_key, None)
            .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    } else {
        return Err(TransportError::MissingLiveConfig);
    }

    if !session.authenticated() {
        return Err(TransportError::UncertainUpload(
            "SFTP authentication failed".to_string(),
        ));
    }

    let sftp = session
        .sftp()
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    let remote_path = format!(
        "{}{}",
        package.manifest.remote_directory, package.manifest.filename
    );
    let mut remote_file = sftp
        .create(std::path::Path::new(&remote_path))
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    remote_file
        .write_all(&package.payload)
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    remote_file
        .flush()
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    Ok(())
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
