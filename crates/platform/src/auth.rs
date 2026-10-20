use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use thiserror::Error;
use unicode_segmentation::UnicodeSegmentation;

const COMMON_PASSWORDS: &[&str] = &[
    "password",
    "password123",
    "password1234",
    "123456789012",
    "qwertyuiop12",
    "letmeinletmein",
    "adminadminadmin",
    "changeme1234",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum PasswordError {
    #[error("password must contain between 12 and 128 characters")]
    InvalidLength,
    #[error("password is too common")]
    CommonPassword,
    #[error("stored password hash is invalid")]
    InvalidHash,
    #[error("password hashing failed")]
    HashingFailed,
}

#[derive(Clone, Debug)]
pub struct PasswordService {
    params: Params,
}

impl PasswordService {
    pub fn new(memory_kib: u32) -> Result<Self, PasswordError> {
        let params =
            Params::new(memory_kib, 2, 1, None).map_err(|_| PasswordError::HashingFailed)?;
        Ok(Self { params })
    }

    /// Fast parameters for tests. Not for production data.
    #[must_use]
    pub fn testing() -> Self {
        Self {
            params: Params::new(32, 1, 1, None).expect("test argon2 parameters"),
        }
    }

    /// Hashes without the policy check. Callers that take user input call `validate` first.
    pub fn hash(&self, password: &str) -> Result<String, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        self.argon2()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| PasswordError::HashingFailed)
    }

    pub fn validate(&self, password: &str) -> Result<(), PasswordError> {
        let length = password.graphemes(true).count();
        if !(12..=128).contains(&length) {
            return Err(PasswordError::InvalidLength);
        }
        if COMMON_PASSWORDS
            .iter()
            .any(|common| password.eq_ignore_ascii_case(common))
        {
            return Err(PasswordError::CommonPassword);
        }
        Ok(())
    }

    pub fn verify(&self, password: &str, encoded_hash: &str) -> Result<bool, PasswordError> {
        let parsed = PasswordHash::new(encoded_hash).map_err(|_| PasswordError::InvalidHash)?;
        Ok(self
            .argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    }

    fn argon2(&self) -> Argon2<'_> {
        Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone())
    }
}

#[must_use]
pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[must_use]
pub fn hash_token(token: &str) -> String {
    hex_encode(Sha256::digest(token.as_bytes()).as_slice())
}

#[must_use]
pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_and_common_passwords() {
        let passwords = PasswordService::testing();
        assert_eq!(
            passwords.validate("short").unwrap_err(),
            PasswordError::InvalidLength
        );
        assert_eq!(
            passwords.validate("password1234").unwrap_err(),
            PasswordError::CommonPassword
        );
    }

    #[test]
    fn hashes_and_verifies() {
        let passwords = PasswordService::testing();
        let hash = passwords.hash("status-demo").unwrap();
        assert!(passwords.verify("status-demo", &hash).unwrap());
        assert!(!passwords.verify("status-other", &hash).unwrap());
    }

    #[test]
    fn token_hash_is_stable_and_not_the_token() {
        let token = generate_token();
        assert_ne!(token, hash_token(&token));
        assert_eq!(hash_token(&token), hash_token(&token));
        assert_eq!(normalize_email("  Ada@Example.com "), "ada@example.com");
    }
}
