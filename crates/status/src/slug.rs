use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlugError {
    #[error("use 1 to 64 lowercase letters, numbers, and single hyphens")]
    Invalid,
}

pub fn validate_slug(slug: &str) -> Result<(), SlugError> {
    let bytes = slug.as_bytes();
    if bytes.is_empty() || bytes.len() > 64 {
        return Err(SlugError::Invalid);
    }
    let mut previous_hyphen = false;
    for (index, byte) in bytes.iter().copied().enumerate() {
        let ok = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !ok || (byte == b'-' && (index == 0 || index + 1 == bytes.len() || previous_hyphen)) {
            return Err(SlugError::Invalid);
        }
        previous_hyphen = byte == b'-';
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_url_slugs() {
        assert!(validate_slug("coollabs").is_ok());
        assert!(validate_slug("coolify-api").is_ok());
        assert!(validate_slug("a").is_ok());
        assert_eq!(validate_slug("Coolify").unwrap_err(), SlugError::Invalid);
        assert_eq!(validate_slug("-a").unwrap_err(), SlugError::Invalid);
        assert_eq!(validate_slug("a--b").unwrap_err(), SlugError::Invalid);
        assert_eq!(validate_slug("a-").unwrap_err(), SlugError::Invalid);
    }
}
