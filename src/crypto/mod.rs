use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::rngs::OsRng;
use rand::RngCore;
use std::convert::TryFrom;
use std::sync::Arc;
use rand_core::TryRngCore;
use crate::Error;

#[derive(Clone)]
pub struct Encryptor {
    cipher: Arc<Aes256Gcm>,
}

impl Encryptor {
    pub fn new(key: &[u8]) -> Result<Self, Error> {
        let key = Key::<Aes256Gcm>::try_from(key)
            .map_err(|e| Error::KeyDerivation(e.to_string()))?;
        let cipher = Aes256Gcm::new(&key);
        Ok(Self {
            cipher: Arc::new(cipher),
        })
    }

    pub fn encrypt(&self, data: &str) -> Result<String, Error> {
        let mut nonce_bytes = [0u8; 12];
        let mut rng = OsRng::default();
        rng.try_fill_bytes(&mut nonce_bytes)
            .map_err(|e| Error::Encryption(e.to_string()))?;

        // Use try_from instead of from_slice
        let nonce = Nonce::try_from(&nonce_bytes[..])
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let ciphertext = self.cipher
            .encrypt(&nonce, data.as_bytes())
            .map_err(|e| Error::Encryption(e.to_string()))?;

        let mut combined = nonce_bytes.to_vec();
        combined.extend(ciphertext);

        Ok(BASE64.encode(combined))
    }

    pub fn decrypt(&self, encrypted_data: &str) -> Result<String, Error> {
        let data = BASE64.decode(encrypted_data)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        if data.len() < 12 {
            return Err(Error::Decryption(
                "Ciphertext too short or nonce missing".into()
            ));
        }

        let (nonce_bytes, ciphertext) = data.split_at(12);

        // Again, use try_from
        let nonce = Nonce::try_from(nonce_bytes)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        let plaintext = self.cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|e| Error::Decryption(e.to_string()))?;

        String::from_utf8(plaintext)
            .map_err(|e| Error::Decryption(e.to_string()))
    }
}
