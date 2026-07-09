//! Deterministic payload transform used by the OSS synthetic fixtures.
//!
//! The transform performs:
//!
//! ```text
//! plaintext pseudo-XML
//!   -> zlib compression at max compression
//!   -> AES-256 key derived from a public synthetic test key material
//!   -> CBC-like stream encryption with no PKCS#7 padding
//! ```
//!
//! This synthetic CBC-like stream has two compatibility-test quirks:
//!
//! 1. `Init(..., InitVector = nil)` sets `IV = AES_encrypt(16 zero bytes)` and
//!    then starts CBC from that encrypted IV.
//! 2. A final partial block is not padded. Instead, it encrypts the current CV
//!    and XORs the partial tail with that keystream prefix.

use aes::Aes256;
use cipher::generic_array::GenericArray;
use cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

const BLOCK_SIZE: usize = 16;
const TEST_KEY_MATERIAL: &[u8] = b"oss-synthetic-fixture-key-material";

/// Errors from synthetic payload compression/decompression.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("zlib compression/decompression failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Encrypt a plaintext synthetic XML field dump into the binary `.xml`
/// payload used by the synthetic packaging flow.
///
/// This is deterministic and covered by synthetic fixture tests.
pub fn encrypt_payload(plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let compressed = zlib_compress_best(plaintext)?;
    Ok(dcpcrypt_aes256_cbc_encrypt_preserve_tail(&compressed))
}

/// Decrypt a synthetic binary `.xml` payload back into the pseudo-XML field
/// dump. This is mostly for fixture verification and diagnostics.
pub fn decrypt_payload(ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let compressed = dcpcrypt_aes256_cbc_decrypt_preserve_tail(ciphertext);
    Ok(zlib_decompress(&compressed)?)
}

fn zlib_compress_best(input: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(input)?;
    encoder.finish()
}

fn zlib_decompress(input: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = ZlibDecoder::new(input);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
}

fn dcpcrypt_key() -> [u8; 32] {
    Sha256::digest(TEST_KEY_MATERIAL).into()
}

fn aes256() -> Aes256 {
    Aes256::new(GenericArray::from_slice(&dcpcrypt_key()))
}

fn encrypt_block(cipher: &Aes256, block: &[u8; BLOCK_SIZE]) -> [u8; BLOCK_SIZE] {
    let mut buf = GenericArray::clone_from_slice(block);
    cipher.encrypt_block(&mut buf);
    buf.into()
}

fn decrypt_block(cipher: &Aes256, block: &[u8; BLOCK_SIZE]) -> [u8; BLOCK_SIZE] {
    let mut buf = GenericArray::clone_from_slice(block);
    cipher.decrypt_block(&mut buf);
    buf.into()
}

fn xor_into_left(left: &mut [u8], right: &[u8]) {
    debug_assert!(left.len() <= right.len());
    for (l, r) in left.iter_mut().zip(right.iter()) {
        *l ^= *r;
    }
}

fn initial_cv(cipher: &Aes256) -> [u8; BLOCK_SIZE] {
    // Synthetic stream initialization: fill IV with zeros, then
    // EncryptECB(IV, IV), then Reset() copies IV into CV.
    encrypt_block(cipher, &[0u8; BLOCK_SIZE])
}

fn dcpcrypt_aes256_cbc_encrypt_preserve_tail(input: &[u8]) -> Vec<u8> {
    let cipher = aes256();
    let mut cv = initial_cv(&cipher);
    let mut out = Vec::with_capacity(input.len());

    let mut chunks = input.chunks_exact(BLOCK_SIZE);
    for chunk in &mut chunks {
        let mut block: [u8; BLOCK_SIZE] = chunk.try_into().expect("chunk is 16 bytes");
        xor_into_left(&mut block, &cv);
        let encrypted = encrypt_block(&cipher, &block);
        out.extend_from_slice(&encrypted);
        cv = encrypted;
    }

    let tail = chunks.remainder();
    if !tail.is_empty() {
        let stream = encrypt_block(&cipher, &cv);
        let mut encrypted_tail = tail.to_vec();
        xor_into_left(&mut encrypted_tail, &stream);
        out.extend_from_slice(&encrypted_tail);
    }

    out
}

fn dcpcrypt_aes256_cbc_decrypt_preserve_tail(input: &[u8]) -> Vec<u8> {
    let cipher = aes256();
    let mut cv = initial_cv(&cipher);
    let mut out = Vec::with_capacity(input.len());

    let mut chunks = input.chunks_exact(BLOCK_SIZE);
    for chunk in &mut chunks {
        let cipher_block: [u8; BLOCK_SIZE] = chunk.try_into().expect("chunk is 16 bytes");
        let mut plain = decrypt_block(&cipher, &cipher_block);
        xor_into_left(&mut plain, &cv);
        out.extend_from_slice(&plain);
        cv = cipher_block;
    }

    let tail = chunks.remainder();
    if !tail.is_empty() {
        let stream = encrypt_block(&cipher, &cv);
        let mut plain_tail = tail.to_vec();
        xor_into_left(&mut plain_tail, &stream);
        out.extend_from_slice(&plain_tail);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_then_decrypt_round_trips_arbitrary_partial_tail() {
        let plaintext = b"<synthetic-form><field name=\"txtMonth\">06</field></synthetic-form>\n";
        let encrypted = encrypt_payload(plaintext).expect("encrypt");
        let decrypted = decrypt_payload(&encrypted).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encryption_is_deterministic_for_synthetic_fixture() {
        let plaintext = b"synthetic deterministic payload";
        let left = encrypt_payload(plaintext).expect("encrypt left");
        let right = encrypt_payload(plaintext).expect("encrypt right");
        assert_eq!(left, right);
        assert_ne!(left, plaintext);
    }
}
