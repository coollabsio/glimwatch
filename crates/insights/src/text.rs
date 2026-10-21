const RESERVED_SLUGS: [&str; 8] = [
    "api", "assets", "go", "health", "images", "links", "login", "public",
];

pub fn clean_host(value: &str) -> Result<String, &'static str> {
    let value = value
        .trim()
        .trim_end_matches('.')
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase();
    if value.is_empty() || value.len() > 253 || value.contains('/') || value.contains(' ') {
        Err("Enter a hostname without a scheme or path.")
    } else {
        Ok(value)
    }
}

pub fn clean_name(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 80 {
        Err("Use 1 to 80 characters.")
    } else {
        Ok(value.to_owned())
    }
}

pub fn clean_slug(value: &str) -> Result<String, &'static str> {
    let value = value.trim().trim_matches('/').to_lowercase();
    if value.is_empty()
        || value.len() > 120
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'))
    {
        Err("Use letters, numbers, dashes, and slashes.")
    } else {
        Ok(value)
    }
}

pub fn clean_public_slug(value: &str) -> Result<String, &'static str> {
    let value = clean_slug(value)?;
    if value.contains('/') {
        Err("The public slug is a single path segment.")
    } else if RESERVED_SLUGS.contains(&value.as_str()) {
        Err("That slug is reserved.")
    } else {
        Ok(value)
    }
}

pub fn clean_destination(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    if !(value.starts_with("https://") || value.starts_with("http://")) || value.len() > 2000 {
        Err("Destination must be an http or https URL.")
    } else if value.contains(['\n', '\r']) {
        Err("Destination cannot contain a line break.")
    } else {
        Ok(value.to_owned())
    }
}

pub fn clean_color(value: &str) -> Result<String, &'static str> {
    let value = value.trim();
    let bytes = value.as_bytes();
    if bytes.len() == 7
        && bytes[0] == b'#'
        && bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(value.to_lowercase())
    } else {
        Err("Use a #rrggbb color.")
    }
}

pub fn slugify(value: &str) -> String {
    let mut out = String::new();
    for character in value.trim().to_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            out.push(character);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(60).collect();
    if out.is_empty() {
        "site".to_owned()
    } else {
        out
    }
}
