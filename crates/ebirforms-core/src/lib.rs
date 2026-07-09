//! Core eBIRForms compatibility logic.

pub mod crypto;
pub mod form;
pub mod job;
pub mod package;
pub mod profile;
pub mod receipt;
pub mod submission;
pub mod transport;

pub use crypto::{decrypt_payload, encrypt_payload, CryptoError};
pub use form::{render_form, FormDefinition, FormError, FormMetadata};
pub use job::{
    run_due_jobs_dry_run, run_due_jobs_live, run_next_job, JobError, JobMode, JobStore,
    SubmissionJob,
};
pub use package::{
    build_submission_package, sha256_hex, PackageError, SubmissionManifest, SubmissionPackage,
};
pub use profile::{
    AppSettings, AppState, AppStateStore, PinVerifier, ProfileError, TaxpayerProfile, Theme,
};
pub use receipt::{
    apply_receipt_to_store, parse_and_apply_receipt, parse_receipt, ReceiptError, ReceiptMetadata,
};
pub use submission::{
    blocks_automatic_retry, submit_with_store, SubmissionError, SubmissionRecord, SubmissionStore,
    SubmitMode,
};
pub use transport::{
    idempotency_key, DryRunTransport, SftpTransport, SubmissionStatus, SubmissionTransport,
    TransportError, TransportReceipt,
};
