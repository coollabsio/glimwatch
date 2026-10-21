/// Coarse browser, OS, and device classification from a User-Agent string.
pub fn parse_user_agent(ua: &str) -> (Option<String>, Option<String>, Option<String>) {
    let browser = if ua.contains("DuckDuckGo/") {
        "DuckDuckGo"
    } else if ua.contains("SamsungBrowser/") {
        "Samsung Internet"
    } else if ua.contains("Vivaldi/") {
        "Vivaldi"
    } else if ua.contains("Edg/") || ua.contains("EdgiOS/") || ua.contains("EdgA/") {
        "Edge"
    } else if ua.contains("OPR/") || ua.contains("OPT/") || ua.contains("Opera") {
        "Opera"
    } else if ua.contains("Firefox/") || ua.contains("FxiOS/") {
        "Firefox"
    } else if ua.contains("Chromium/") {
        "Chromium"
    } else if ua.contains("Chrome/") || ua.contains("CriOS/") {
        "Chrome"
    } else if ua.contains("Safari/") {
        "Safari"
    } else if ua.contains("Trident/") || ua.contains("MSIE ") {
        "Internet Explorer"
    } else {
        "Other"
    };
    let os = if ua.contains("Windows") {
        "Windows"
    } else if ua.contains("Android") {
        "Android"
    } else if ua.contains("iPhone") || ua.contains("iPad") {
        "iOS"
    } else if ua.contains("Mac OS") {
        "macOS"
    } else if ua.contains("Linux") {
        "Linux"
    } else {
        "Other"
    };
    let device = if ua.contains("iPad")
        || ua.contains("Tablet")
        || (ua.contains("Android") && !ua.contains("Mobile"))
    {
        "Tablet"
    } else if ua.contains("Mobile") || ua.contains("iPhone") {
        "Mobile"
    } else {
        "Desktop"
    };
    (
        Some(browser.to_owned()),
        Some(os.to_owned()),
        Some(device.to_owned()),
    )
}
