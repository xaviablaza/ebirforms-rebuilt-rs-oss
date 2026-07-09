//! Core eBIRForms compatibility logic.

pub mod crypto;

pub use crypto::{decrypt_payload, encrypt_payload, CryptoError};
