fn main() {
    for key in [
        "BIR_SFTP_HOST",
        "BIR_SFTP_PORT",
        "BIR_SFTP_USERNAME",
        "BIR_SFTP_PASSWORD",
        "BIR_SFTP_PRIVATE_KEY",
        "BIR_SFTP_KNOWN_HOSTS",
        "BIR_SFTP_ACCEPT_UNKNOWN_HOST",
        "BIR_SFTP_BACKEND",
        "FILING_SFTP_HOST",
        "FILING_SFTP_PORT",
        "FILING_SFTP_USERNAME",
        "FILING_SFTP_PASSWORD",
        "FILING_SFTP_PRIVATE_KEY",
        "FILING_SFTP_KNOWN_HOSTS",
        "BIR_PRODUCTION_SFTP_HOST",
        "BIR_PRODUCTION_SFTP_PORT",
        "BIR_PRODUCTION_SFTP_USERNAME",
        "BIR_PRODUCTION_SFTP_PASSWORD",
        "BIR_PRODUCTION_SFTP_PRIVATE_KEY",
        "BIR_PRODUCTION_SFTP_KNOWN_HOSTS",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
}
