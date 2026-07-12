use crate::package::SubmissionPackage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::Write;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    /// If true, pass `StrictHostKeyChecking=accept-new` to OpenSSH. Prefer
    /// `known_hosts` for production; this exists to match the legacy helper's
    /// permissive first-connect behavior without disabling host-key checks
    /// entirely.
    pub accept_unknown_host: bool,
    /// Upload backend. Default is `openssh` for portable Linux operation.
    /// Set `BIR_SFTP_BACKEND=winscp` when reproducing the official eBIRForms
    /// path: WinSCP SFTP over SSH via Wine, with a temporary script file that
    /// holds credentials outside argv and is removed after execution.
    pub backend: SftpBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SftpBackend {
    OpenSsh,
    WinScp,
    NativeSsh2,
}

impl SftpBackend {
    fn from_env() -> Self {
        match std::env::var("BIR_SFTP_BACKEND")
            .unwrap_or_else(|_| "openssh".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "winscp" | "winscp-wine" | "wine-winscp" => Self::WinScp,
            "native" | "rust" | "ssh2" | "native-ssh2" => Self::NativeSsh2,
            _ => Self::OpenSsh,
        }
    }
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
        let accept_unknown_host = std::env::var("BIR_SFTP_ACCEPT_UNKNOWN_HOST")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);
        let backend = SftpBackend::from_env();
        Some(Self {
            host,
            port,
            username,
            password,
            private_key,
            known_hosts,
            accept_unknown_host,
            backend,
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
        match config.backend {
            SftpBackend::OpenSsh => submit_with_openssh(config, package)?,
            SftpBackend::WinScp => submit_with_winscp(config, package)?,
            SftpBackend::NativeSsh2 => submit_with_native_ssh2(config, package)?,
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

fn submit_with_openssh(
    config: &SftpConfig,
    package: &SubmissionPackage,
) -> Result<(), TransportError> {
    let staged_path = stage_payload(package)?;

    // Use the system sftp client. If password auth is configured, wrap it
    // with `sshpass -e` so the password is supplied via SSHPASS rather than
    // appearing in argv. Key-based auth uses plain `sftp`.
    let remote_target = format!(
        "{}@{}:{}",
        config.username, config.host, package.manifest.remote_directory
    );
    let mut command = if let Some(password) = &config.password {
        let mut command = Command::new("sshpass");
        command.arg("-e").arg("sftp");
        command.env("SSHPASS", password);
        command
    } else {
        Command::new("sftp")
    };
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
    if config.accept_unknown_host {
        command.arg("-o").arg("StrictHostKeyChecking=accept-new");
    }
    if config.password.is_some() {
        // OpenSSH's sftp batch mode otherwise behaves like BatchMode=yes and
        // will not prompt, so sshpass never gets to supply the password.
        command
            .arg("-o")
            .arg("BatchMode=no")
            .arg("-o")
            .arg("NumberOfPasswordPrompts=1");
    }
    command.arg(remote_target);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
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

    Ok(())
}

fn submit_with_winscp(
    config: &SftpConfig,
    package: &SubmissionPackage,
) -> Result<(), TransportError> {
    let password = config
        .password
        .as_ref()
        .ok_or(TransportError::MissingLiveConfig)?;
    let staged_path = stage_payload(package)?;
    let script_path = temporary_path("ebirforms-winscp", "txt");
    let log_path = temporary_path("ebirforms-winscp", "log");

    // This mirrors the official eBIRForms transport more closely than OpenSSH:
    // WinSCP SFTP over SSH, safe SFTP v3 negotiation, binary transfer, and no
    // attempt to create the remote form directory (BIR pre-creates it; mkdir can
    // fail even when upload is allowed). Credentials live only in this chmod 600
    // temporary script and are not placed on the process command line.
    let local_path = windows_path(&staged_path);
    let script = format!(
        "option batch abort\noption confirm off\nopen sftp://{}:{}/ -username=\"{}\" -password=\"{}\" -hostkey=*\ncd {}\noption transfer binary\nput \"{}\" \"{}\"\nexit\n",
        config.host,
        config.port,
        winscp_escape(&config.username),
        winscp_escape(password),
        package.manifest.remote_directory,
        winscp_escape(&local_path),
        winscp_escape(&package.manifest.filename)
    );
    std::fs::write(&script_path, script).map_err(|err| TransportError::Staging(err.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o600));
    }

    let winscp_exe = std::env::var("BIR_WINSCP_EXE")
        .unwrap_or_else(|_| "/home/vettel/ebirforms-binaries/WinSCP.exe".to_string());
    let wine_cmd = std::env::var("BIR_WINE_CMD").unwrap_or_else(|_| "wine".to_string());
    let output = Command::new(wine_cmd)
        .arg(winscp_exe)
        .arg("/ini=nul")
        .arg(format!("/log={}", windows_path(&log_path)))
        .arg(format!("/script={}", windows_path(&script_path)))
        .env("WINEDEBUG", "-all")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;

    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&staged_path);

    if !output.status.success() {
        let mut detail = String::from_utf8_lossy(&output.stderr).to_string();
        if detail.trim().is_empty() {
            detail = std::fs::read_to_string(&log_path).unwrap_or_default();
        }
        let _ = std::fs::remove_file(&log_path);
        return Err(TransportError::UncertainUpload(redact_transport_error(
            &detail,
        )));
    }

    let _ = std::fs::remove_file(&log_path);
    Ok(())
}

fn submit_with_native_ssh2(
    config: &SftpConfig,
    package: &SubmissionPackage,
) -> Result<(), TransportError> {
    let password = config
        .password
        .as_ref()
        .ok_or(TransportError::MissingLiveConfig)?;
    let tcp = TcpStream::connect((config.host.as_str(), config.port))
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    let _ = tcp.set_read_timeout(Some(Duration::from_secs(30)));
    let _ = tcp.set_write_timeout(Some(Duration::from_secs(30)));

    let mut session =
        ssh2::Session::new().map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    session.set_tcp_stream(tcp);
    session.set_timeout(30_000);
    session
        .handshake()
        .map_err(|err| TransportError::UncertainUpload(redact_transport_error(&err.to_string())))?;
    session
        .userauth_password(&config.username, password)
        .map_err(|err| TransportError::UncertainUpload(redact_transport_error(&err.to_string())))?;
    if !session.authenticated() {
        return Err(TransportError::UncertainUpload(
            "native ssh2 authentication failed".to_string(),
        ));
    }

    let sftp = session
        .sftp()
        .map_err(|err| TransportError::UncertainUpload(redact_transport_error(&err.to_string())))?;
    let mut remote = sftp
        .create(Path::new(&package.manifest.remote_path))
        .map_err(|err| TransportError::UncertainUpload(redact_transport_error(&err.to_string())))?;
    remote
        .write_all(&package.payload)
        .map_err(|err| TransportError::UncertainUpload(err.to_string()))?;
    remote
        .close()
        .map_err(|err| TransportError::UncertainUpload(redact_transport_error(&err.to_string())))?;
    Ok(())
}

fn stage_payload(package: &SubmissionPackage) -> Result<PathBuf, TransportError> {
    let staged_path = std::env::temp_dir().join(format!("ebirforms-{}", package.manifest.filename));
    std::fs::write(&staged_path, &package.payload)
        .map_err(|err| TransportError::Staging(err.to_string()))?;
    Ok(staged_path)
}

fn temporary_path(prefix: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!(
        "{}-{}-{}.{}",
        prefix,
        std::process::id(),
        nanos,
        extension
    ))
}

fn windows_path(path: &Path) -> String {
    let text = path.display().to_string();
    if let Some(stripped) = text.strip_prefix('/') {
        format!("Z:\\{}", stripped.replace('/', "\\"))
    } else {
        text.replace('/', "\\")
    }
}

fn winscp_escape(value: &str) -> String {
    value.replace('"', "\"\"")
}

pub fn idempotency_key(package: &SubmissionPackage) -> String {
    format!(
        "{}:{}:{}:{}",
        package.manifest.form_code,
        package.manifest.period_mm_yyyy,
        package.manifest.remote_path,
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

    #[test]
    fn winscp_windows_path_uses_wine_z_drive_mapping() {
        assert_eq!(
            windows_path(Path::new("/tmp/ebirforms-upload.xml")),
            "Z:\\tmp\\ebirforms-upload.xml"
        );
    }

    #[test]
    fn winscp_escaping_doubles_quotes_for_script_values() {
        assert_eq!(winscp_escape("a\"b"), "a\"\"b");
    }
}
