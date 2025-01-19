use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::rngs::OsRng;
use rand::RngCore;
use std::convert::TryFrom;

#[derive(thiserror::Error, Debug)]
pub enum CryptoError {
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Key derivation error: {0}")]
    KeyDerivation(String),
}

pub struct Encryptor {
    cipher: Aes256Gcm,
}

impl Encryptor {
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        let key = Key::<Aes256Gcm>::try_from(key)
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;
        let cipher = Aes256Gcm::new(&key);
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, data: &str) -> Result<String, CryptoError> {
        let mut nonce_bytes = [0u8; 12];
        let mut rng = OsRng;
        rng.fill_bytes(&mut nonce_bytes);

        // Use try_from instead of from_slice
        let nonce = Nonce::try_from(&nonce_bytes[..])
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        let ciphertext = self.cipher
            .encrypt(&nonce, data.as_bytes())
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        let mut combined = nonce_bytes.to_vec();
        combined.extend(ciphertext);

        Ok(BASE64.encode(combined))
    }

    pub fn decrypt(&self, encrypted_data: &str) -> Result<String, CryptoError> {
        let data = BASE64.decode(encrypted_data)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        if data.len() < 12 {
            return Err(CryptoError::Decryption(
                "Ciphertext too short or nonce missing".into()
            ));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);

        // Again, use try_from
        let nonce = Nonce::try_from(nonce_bytes)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        let plaintext = self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }
}
