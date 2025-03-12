use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};

/// CredentialType now supports only the known types.
/// We have removed the old data-bearing custom variant and replaced it
/// with a unit variant for interactive 2FA.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth2,
    APIKey,
    BearerToken,
    JWT,
    VerifiableCredential,
    Interactive2FA,
}

impl fmt::Display for CredentialType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CredentialType::OAuth2 => write!(f, "oauth2"),
            CredentialType::APIKey => write!(f, "apikey"),
            CredentialType::BearerToken => write!(f, "bearer"),
            CredentialType::JWT => write!(f, "jwt"),
            CredentialType::VerifiableCredential => write!(f, "vc"),
            CredentialType::Interactive2FA => write!(f, "interactive2fa"),
        }
    }
}

impl FromStr for CredentialType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oauth2" => Ok(CredentialType::OAuth2),
            "apikey" => Ok(CredentialType::APIKey),
            "bearer" => Ok(CredentialType::BearerToken),
            "jwt" => Ok(CredentialType::JWT),
            "vc" => Ok(CredentialType::VerifiableCredential),
            "interactive2fa" | "i2fa" => Ok(CredentialType::Interactive2FA),
            _ => Err(format!("Invalid credential type: {}", s))
        }
    }
}