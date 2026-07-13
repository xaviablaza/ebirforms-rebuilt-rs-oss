use crate::package::SubmissionPackage;
use serde::{Deserialize, Serialize};
use ssh2::Session;
use std::collections::BTreeSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("live SFTP transport requires configured BIR_SFTP_* or FILING_SFTP_* environment variables, or embedded build-time SFTP defaults")]
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
        let host = env_first(&["BIR_SFTP_HOST", "FILING_SFTP_HOST"])?;
        let username = env_first(&["BIR_SFTP_USERNAME", "FILING_SFTP_USERNAME"])?;
        let port = env_first(&["BIR_SFTP_PORT", "FILING_SFTP_PORT"])
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or(23);
        let password = env_first(&["BIR_SFTP_PASSWORD", "FILING_SFTP_PASSWORD"]);
        let private_key =
            env_first(&["BIR_SFTP_PRIVATE_KEY", "FILING_SFTP_PRIVATE_KEY"]).map(PathBuf::from);
        let known_hosts =
            env_first(&["BIR_SFTP_KNOWN_HOSTS", "FILING_SFTP_KNOWN_HOSTS"]).map(PathBuf::from);
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
        let host = build_time_first(&[
            "BIR_SFTP_HOST",
            "FILING_SFTP_HOST",
            "BIR_PRODUCTION_SFTP_HOST",
        ])?;
        let username = build_time_first(&[
            "BIR_SFTP_USERNAME",
            "FILING_SFTP_USERNAME",
            "BIR_PRODUCTION_SFTP_USERNAME",
        ])?;
        let password = build_time_first(&[
            "BIR_SFTP_PASSWORD",
            "FILING_SFTP_PASSWORD",
            "BIR_PRODUCTION_SFTP_PASSWORD",
        ]);
        let private_key = build_time_first(&[
            "BIR_SFTP_PRIVATE_KEY",
            "FILING_SFTP_PRIVATE_KEY",
            "BIR_PRODUCTION_SFTP_PRIVATE_KEY",
        ])
        .map(PathBuf::from);
        if password.is_none() && private_key.is_none() {
            return None;
        }
        Some(Self {
            host,
            port: build_time_first(&[
                "BIR_SFTP_PORT",
                "FILING_SFTP_PORT",
                "BIR_PRODUCTION_SFTP_PORT",
            ])
            .as_deref()
            .and_then(|value| value.parse().ok())
            .unwrap_or(23),
            username,
            password,
            private_key,
            known_hosts: build_time_first(&[
                "BIR_SFTP_KNOWN_HOSTS",
                "FILING_SFTP_KNOWN_HOSTS",
                "BIR_PRODUCTION_SFTP_KNOWN_HOSTS",
            ])
            .map(PathBuf::from),
        })
    }
}

fn build_time_first(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| embedded_build_time_env(name))
        .filter(|value| !value.is_empty())
}

fn embedded_build_time_env(name: &str) -> Option<String> {
    embedded_bir_sftp_env(name).or_else(|| embedded_production_sftp_env(name))
}

#[cfg(feature = "embed-bir-sftp-secrets")]
fn embedded_bir_sftp_env(name: &str) -> Option<String> {
    match name {
        "BIR_SFTP_HOST" => option_env!("BIR_SFTP_HOST"),
        "BIR_SFTP_PORT" => option_env!("BIR_SFTP_PORT"),
        "BIR_SFTP_USERNAME" => option_env!("BIR_SFTP_USERNAME"),
        "BIR_SFTP_PASSWORD" => option_env!("BIR_SFTP_PASSWORD"),
        "BIR_SFTP_PRIVATE_KEY" => option_env!("BIR_SFTP_PRIVATE_KEY"),
        "BIR_SFTP_KNOWN_HOSTS" => option_env!("BIR_SFTP_KNOWN_HOSTS"),
        "FILING_SFTP_HOST" => option_env!("FILING_SFTP_HOST"),
        "FILING_SFTP_PORT" => option_env!("FILING_SFTP_PORT"),
        "FILING_SFTP_USERNAME" => option_env!("FILING_SFTP_USERNAME"),
        "FILING_SFTP_PASSWORD" => option_env!("FILING_SFTP_PASSWORD"),
        "FILING_SFTP_PRIVATE_KEY" => option_env!("FILING_SFTP_PRIVATE_KEY"),
        "FILING_SFTP_KNOWN_HOSTS" => option_env!("FILING_SFTP_KNOWN_HOSTS"),
        _ => None,
    }
    .map(str::to_owned)
}

#[cfg(not(feature = "embed-bir-sftp-secrets"))]
fn embedded_bir_sftp_env(_name: &str) -> Option<String> {
    None
}

fn embedded_production_sftp_env(name: &str) -> Option<String> {
    match name {
        "BIR_PRODUCTION_SFTP_HOST" => option_env!("BIR_PRODUCTION_SFTP_HOST"),
        "BIR_PRODUCTION_SFTP_PORT" => option_env!("BIR_PRODUCTION_SFTP_PORT"),
        "BIR_PRODUCTION_SFTP_USERNAME" => option_env!("BIR_PRODUCTION_SFTP_USERNAME"),
        "BIR_PRODUCTION_SFTP_PASSWORD" => option_env!("BIR_PRODUCTION_SFTP_PASSWORD"),
        "BIR_PRODUCTION_SFTP_PRIVATE_KEY" => option_env!("BIR_PRODUCTION_SFTP_PRIVATE_KEY"),
        "BIR_PRODUCTION_SFTP_KNOWN_HOSTS" => option_env!("BIR_PRODUCTION_SFTP_KNOWN_HOSTS"),
        _ => None,
    }
    .map(str::to_owned)
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

fn env_first(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
    })
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

    #[test]
    fn bir_sftp_runtime_env_takes_precedence_over_legacy_filing_env() {
        unsafe {
            std::env::set_var("BIR_SFTP_HOST", "bir.example.test");
            std::env::set_var("BIR_SFTP_PORT", "123");
            std::env::set_var("BIR_SFTP_USERNAME", "bir-user");
            std::env::set_var("BIR_SFTP_PASSWORD", "bir-pass");
            std::env::set_var("FILING_SFTP_HOST", "legacy.example.test");
            std::env::set_var("FILING_SFTP_USERNAME", "legacy-user");
        }

        let config = SftpConfig::from_env().unwrap();

        assert_eq!(config.host, "bir.example.test");
        assert_eq!(config.port, 123);
        assert_eq!(config.username, "bir-user");
        assert_eq!(config.password.as_deref(), Some("bir-pass"));

        unsafe {
            for key in [
                "BIR_SFTP_HOST",
                "BIR_SFTP_PORT",
                "BIR_SFTP_USERNAME",
                "BIR_SFTP_PASSWORD",
                "FILING_SFTP_HOST",
                "FILING_SFTP_USERNAME",
            ] {
                std::env::remove_var(key);
            }
        }
    }
}
