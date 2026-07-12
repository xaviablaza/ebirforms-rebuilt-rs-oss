//! Compatibility implementation for the legacy `Encrypt.exe` helper bundled with
//! eBIRForms.
//!
//! The observed helper performs:
//!
//! ```text
//! plaintext pseudo-XML
//!   -> zlib compression at max compression
//!   -> DCPcrypt `TDCP_rijndael.InitStr(passphrase, TDCP_sha256)`
//!   -> DCPcrypt CBC stream encryption with no PKCS#7 padding
//! ```
//!
//! DCPcrypt's 128-bit block-cipher CBC implementation has two important quirks:
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
const PASSPHRASE: &[u8] = b"T0081gP45sy0rd-To+R3m3m63r!@4/<>";

/// Errors from eBIRForms payload compression/decompression.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("zlib compression/decompression failed: {0}")]
    Io(#[from] std::io::Error),
}

/// Encrypt a plaintext eBIRForms pseudo-XML field dump into the binary `.xml`
/// payload accepted by the legacy BIR upload flow.
///
/// This is intended to be byte-compatible with the captured `Encrypt.exe` for
/// the tested 1601C fixture.
pub fn encrypt_payload(plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let compressed = zlib_compress_best(plaintext)?;
    Ok(dcpcrypt_aes256_cbc_encrypt_preserve_tail(&compressed))
}

/// Decrypt an eBIRForms binary `.xml` payload back into the pseudo-XML field
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
    Sha256::digest(PASSPHRASE).into()
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
    // DCPcrypt TDCP_blockcipher128.Init(..., nil): fill IV with zeros, then
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
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/private/1601c")
            .join(name)
    }

    fn read_private_fixture(name: &str) -> Option<Vec<u8>> {
        let path = fixture_path(name);
        std::fs::read(&path).ok()
    }

    #[test]
    fn encrypt_matches_private_1601c_v2_artifact_when_available() {
        let Some(plaintext) = read_private_fixture("plaintext-v2.xml") else {
            return;
        };
        let encrypted = encrypt_payload(&plaintext).expect("encrypt fixture");

        assert_eq!(encrypted.len(), 956);
        assert_eq!(
            sha256_hex(&encrypted),
            "8b3ef7fb4a60eb765a4da24f79ad7a7850965171bdec049523cd68509693648f"
        );

        if let Some(expected) = read_private_fixture("encrypted-v2.xml") {
            assert_eq!(encrypted, expected);
        }
    }

    #[test]
    fn decrypt_round_trips_private_1601c_v2_artifact_when_available() {
        let Some(encrypted) = read_private_fixture("encrypted-v2.xml") else {
            return;
        };
        let plaintext = decrypt_payload(&encrypted).expect("decrypt fixture");

        assert_eq!(plaintext.len(), 5571);
        assert_eq!(
            sha256_hex(&plaintext),
            "c43f00e60ede596093112f9f806842fba5ab8bdcfc3ed384bdfcf14e268d6713"
        );

        if let Some(expected) = read_private_fixture("plaintext-v2.xml") {
            assert_eq!(plaintext, expected);
        }
    }

    #[test]
    fn encrypt_then_decrypt_round_trips_arbitrary_partial_tail() {
        // Exercises non-block-aligned input after compression and the DCPcrypt
        // final-tail behavior. This is not a real tax form.
        let plaintext = b"<div>frm1601c:txtMonth=06frm1601c:txtMonth=</div>\r\n";
        let encrypted = encrypt_payload(plaintext).expect("encrypt");
        let decrypted = decrypt_payload(&encrypted).expect("decrypt");
        assert_eq!(decrypted, plaintext);
    }
}
