use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::rngs::OsRng;
use rand_core::TryRngCore;
use std::sync::Arc;

use crate::Error;

#[derive(Clone)]
pub struct Encryptor {
    cipher: Arc<Aes256Gcm>,
}

impl Encryptor {
    /// Creates a new `Encryptor` using a 32‐byte key for AES‐256.
    pub fn new(key_bytes: &[u8]) -> Result<Self, Error> {
        // AES-256-GCM requires a 256-bit (32 bytes) key.
        if key_bytes.len() != 32 {
            return Err(Error::KeyDerivation(
                format!("AES-256 key must be 32 bytes, got {}", key_bytes.len())
            ));
        }
        // Safely clone bytes into a `Key`.
        let key = Key::<Aes256Gcm>::clone_from_slice(key_bytes);

        // Initialize the AES-GCM cipher.
        let cipher = Aes256Gcm::new(&key);

        Ok(Self {
            cipher: Arc::new(cipher),
        })
    }

    /// Encrypts `data` into base64(`nonce || ciphertext`).
    ///
    /// - A random 12‐byte nonce is generated each time (for AES-GCM).
    /// - `data` is then encrypted with that nonce and the configured key.
    pub fn encrypt(&self, data: &str) -> Result<String, Error> {
        let mut nonce_bytes = [0u8; 12];
        let mut rng = OsRng;
        rng.try_fill_bytes(&mut nonce_bytes)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Convert our 12‐byte array into an AES-GCM `Nonce`.
        // Note: `from_slice` returns `&Nonce`; we can pass it to encrypt directly.
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt the data. On failure, map the error to our custom `Error`.
        let ciphertext = self.cipher
            .encrypt(nonce, data.as_bytes())
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Combine nonce + ciphertext for storage/transmission:
        let mut combined = nonce_bytes.to_vec();
        combined.extend(ciphertext);

        // Encode as base64.
        Ok(BASE64.encode(combined))
    }

    /// Decrypts base64(`nonce || ciphertext`) back into a `String`.
    pub fn decrypt(&self, encrypted_data: &str) -> Result<String, Error> {
        let data = BASE64.decode(encrypted_data)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        // The first 12 bytes are the nonce.
        if data.len() < 12 {
            return Err(Error::Decryption(
                "Ciphertext too short (missing nonce)".to_owned()
            ));
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);

        // Convert the nonce bytes to a `Nonce`. (12 bytes required for AES-GCM.)
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt with AES-GCM. On failure, map the error to our custom `Error`.
        let plaintext = self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        // Convert decrypted bytes to UTF-8 text.
        String::from_utf8(plaintext)
            .map_err(|e| Error::Decryption(e.to_string()))
    }
}
