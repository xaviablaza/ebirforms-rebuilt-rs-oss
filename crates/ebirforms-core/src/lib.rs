//! Core eBIRForms compatibility logic.

pub mod crypto;
pub mod form;
pub mod package;
pub mod submission;
pub mod transport;

pub use crypto::{decrypt_payload, encrypt_payload, CryptoError};
pub use form::{render_form, FormDefinition, FormError, FormMetadata};
pub use package::{
    build_submission_package, sha256_hex, PackageError, SubmissionManifest, SubmissionPackage,
};
pub use submission::{
    blocks_automatic_retry, submit_with_store, SubmissionError, SubmissionRecord, SubmissionStore,
    SubmitMode,
};
pub use transport::{
    idempotency_key, DryRunTransport, SftpTransport, SubmissionStatus, SubmissionTransport,
    TransportError, TransportReceipt,
};
