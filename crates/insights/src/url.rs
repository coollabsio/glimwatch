//! Splits a page URL the way Umami does: path (with hash), query string, UTM parameters.

pub struct PageUrl {
    pub path: String,
    pub query: Option<String>,
    pub utm: [Option<String>; 5],
}

const UTM: [&str; 5] = [
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_content",
    "utm_term",
];

pub fn parse(raw: &str) -> Option<PageUrl> {
    let raw = raw.trim();
    let rest = match raw.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("/", |index| &rest[index..]),
        None => raw,
    };
    if !rest.starts_with('/') {
        return None;
    }
    let (rest, hash) = rest
        .split_once('#')
        .map_or((rest, None), |(path, hash)| (path, Some(hash)));
    let (path, query) = rest
        .split_once('?')
        .map_or((rest, None), |(path, query)| (path, Some(query)));
    let mut path = percent_decode(path);
    if let Some(hash) = hash.filter(|hash| !hash.is_empty()) {
        path.push('#');
        path.push_str(&percent_decode(hash));
    }
    let query = query
        .filter(|query| !query.is_empty())
        .map(|query| clip(query, 500));
    let mut utm: [Option<String>; 5] = Default::default();
    for pair in query.as_deref().unwrap_or("").split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if let Some(index) = UTM.iter().position(|name| *name == key) {
            let value = percent_decode(&value.replace('+', " "));
            if !value.is_empty() {
                utm[index] = Some(clip(&value, 255));
            }
        }
    }
    Some(PageUrl {
        path: clip(&path, 500),
        query,
        utm,
    })
}

pub fn clip(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

pub fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let decoded = (bytes[index] == b'%' && index + 2 < bytes.len())
            .then(|| std::str::from_utf8(&bytes[index + 1..index + 3]).ok())
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match decoded {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Lowercase host without scheme, port, or a leading `www.`.
pub fn referrer_domain(value: &str) -> Option<String> {
    let rest = value.split_once("://")?.1;
    let host = rest.split('/').next()?.split(':').next()?.to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    (!host.is_empty() && host.len() <= 253).then_some(host)
}

/// `host` is the site's domain or one of its subdomains.
pub fn same_site(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}
