use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedEvent {
    pub event_id: String,
    pub timestamp: String,
    pub level: String,
    pub title: String,
    pub message: Option<String>,
    pub culprit: Option<String>,
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
    pub server_name: Option<String>,
    pub transaction: Option<String>,
    pub logger: Option<String>,
    pub fingerprint: String,
    pub sdk: Option<SdkInfo>,
    pub exceptions: Vec<ExceptionInfo>,
    pub breadcrumbs: Vec<BreadcrumbInfo>,
    pub tags: Vec<Tag>,
    pub contexts: Value,
    pub user: Value,
    pub request: Value,
    pub extra: Value,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SdkInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionInfo {
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub value: Option<String>,
    pub mechanism: Option<String>,
    pub handled: Option<bool>,
    pub frames: Vec<FrameInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameInfo {
    pub filename: Option<String>,
    pub abs_path: Option<String>,
    pub function: Option<String>,
    pub module: Option<String>,
    pub lineno: Option<u32>,
    pub colno: Option<u32>,
    pub context_line: Option<String>,
    pub pre_context: Vec<String>,
    pub post_context: Vec<String>,
    pub in_app: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BreadcrumbInfo {
    pub timestamp: Option<String>,
    #[serde(rename = "type")]
    pub type_name: Option<String>,
    pub category: Option<String>,
    pub level: Option<String>,
    pub message: Option<String>,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub key: String,
    pub value: String,
}

pub fn normalize_event(raw: Value) -> NormalizedEvent {
    let event = if raw.is_object() {
        raw
    } else {
        serde_json::json!({ "message": "Unparsed event", "extra": { "payload": raw } })
    };
    let mut exceptions = parse_exceptions(&event);
    attach_thread_stack(&event, &mut exceptions);
    let message = message_of(&event);
    let title = make_title(&exceptions, message.as_deref());
    let culprit = culprit_of(&event, &exceptions);
    let tags = parse_tags(&event);
    let breadcrumbs = parse_breadcrumbs(&event);
    let sdk = parse_sdk(&event);
    let fingerprint = fingerprint_of(&event, &exceptions, message.as_deref());
    NormalizedEvent {
        event_id: event_id_of(&event),
        timestamp: timestamp_of(event.get("timestamp")),
        level: level_of(event.get("level").and_then(Value::as_str)),
        title,
        message,
        culprit,
        platform: string_field(&event, "platform"),
        environment: string_field(&event, "environment"),
        release: string_field(&event, "release"),
        server_name: string_field(&event, "server_name"),
        transaction: string_field(&event, "transaction"),
        logger: string_field(&event, "logger"),
        fingerprint,
        sdk,
        exceptions,
        breadcrumbs,
        tags,
        contexts: object_field(&event, "contexts"),
        user: object_field(&event, "user"),
        request: object_field(&event, "request"),
        extra: object_field(&event, "extra"),
        raw: event,
    }
}

fn event_id_of(event: &Value) -> String {
    if let Some(id) = event.get("event_id").and_then(Value::as_str) {
        let cleaned: String = id.chars().filter(|ch| *ch != '-').collect();
        if !cleaned.is_empty() {
            return cleaned.to_ascii_lowercase();
        }
    }
    Uuid::new_v4().simple().to_string()
}

fn timestamp_of(value: Option<&Value>) -> String {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let Some(value) = value else {
        return now;
    };
    if let Some(seconds) = value.as_f64() {
        return unix_to_rfc3339(seconds).unwrap_or(now);
    }
    if let Some(seconds) = value.as_i64() {
        return unix_to_rfc3339(seconds as f64).unwrap_or(now);
    }
    if let Some(text) = value.as_str() {
        if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
            return parsed
                .with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true);
        }
        if let Ok(seconds) = text.parse::<f64>() {
            return unix_to_rfc3339(seconds).unwrap_or(now);
        }
        if !text.is_empty() {
            return text.to_owned();
        }
    }
    now
}

fn unix_to_rfc3339(seconds: f64) -> Option<String> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    let whole = seconds.trunc() as i64;
    let nanos = ((seconds.fract()) * 1_000_000_000.0).round() as u32;
    DateTime::from_timestamp(whole, nanos.min(999_999_999))
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn level_of(value: Option<&str>) -> String {
    match value.unwrap_or("error").to_ascii_lowercase().as_str() {
        "fatal" => "fatal",
        "warning" => "warning",
        "info" => "info",
        "debug" => "debug",
        _ => "error",
    }
    .to_owned()
}

fn string_field(event: &Value, key: &str) -> Option<String> {
    event
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn object_field(event: &Value, key: &str) -> Value {
    match event.get(key) {
        Some(Value::Object(map)) => Value::Object(map.clone()),
        _ => Value::Object(Default::default()),
    }
}

fn message_of(event: &Value) -> Option<String> {
    text_message(event.get("message"))
        .or_else(|| text_message(event.get("logentry")))
        .filter(|value| !value.is_empty())
}

fn text_message(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Object(map) => map
            .get("formatted")
            .and_then(Value::as_str)
            .or_else(|| map.get("message").and_then(Value::as_str))
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn make_title(exceptions: &[ExceptionInfo], message: Option<&str>) -> String {
    let full = if let Some(exception) = exceptions.last() {
        match (&exception.type_name, &exception.value) {
            (Some(type_name), Some(value)) => format!("{type_name}: {value}"),
            (Some(type_name), None) => type_name.clone(),
            (None, Some(value)) => value.clone(),
            (None, None) => message.unwrap_or("Error").to_owned(),
        }
    } else {
        message.unwrap_or("Error").to_owned()
    };
    truncate(&full, 300)
}

fn culprit_of(event: &Value, exceptions: &[ExceptionInfo]) -> Option<String> {
    if let Some(transaction) = string_field(event, "transaction") {
        return Some(transaction);
    }
    let frames = exceptions
        .last()
        .map(|exception| relevant_frames(&exception.frames))
        .unwrap_or_default();
    frames.last().and_then(|frame| {
        frame
            .function
            .clone()
            .or_else(|| frame.filename.clone())
            .or_else(|| frame.abs_path.clone())
            .or_else(|| frame.module.clone())
    })
}

fn parse_exceptions(event: &Value) -> Vec<ExceptionInfo> {
    let Some(exception) = event.get("exception") else {
        return Vec::new();
    };
    let values: Vec<&Value> =
        if let Some(values) = exception.get("values").and_then(Value::as_array) {
            values.iter().collect()
        } else if exception.get("type").is_some() || exception.get("value").is_some() {
            vec![exception]
        } else {
            Vec::new()
        };
    values.iter().map(|value| parse_exception(value)).collect()
}

fn parse_exception(value: &Value) -> ExceptionInfo {
    let mechanism = value.get("mechanism");
    let (mechanism_type, handled) = match mechanism {
        Some(Value::Object(map)) => (
            map.get("type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            map.get("handled").and_then(bool_of),
        ),
        Some(Value::String(text)) => (Some(text.clone()), None),
        _ => (None, None),
    };
    let frames = value
        .get("stacktrace")
        .and_then(|stack| stack.get("frames"))
        .and_then(Value::as_array)
        .map(|frames| frames.iter().map(parse_frame).collect())
        .unwrap_or_default();
    ExceptionInfo {
        type_name: value
            .get("type")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        value: value
            .get("value")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        mechanism: mechanism_type,
        handled,
        frames,
    }
}

fn parse_frame(value: &Value) -> FrameInfo {
    let mut frame = FrameInfo {
        filename: value
            .get("filename")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        abs_path: value
            .get("abs_path")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        function: value
            .get("function")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        module: value
            .get("module")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        lineno: value.get("lineno").and_then(u32_of),
        colno: value.get("colno").and_then(u32_of),
        context_line: value
            .get("context_line")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        pre_context: string_list(value.get("pre_context")),
        post_context: string_list(value.get("post_context")),
        in_app: value.get("in_app").and_then(bool_of).unwrap_or(false),
    };
    if is_runtime_frame(&frame) {
        frame.in_app = false;
    }
    frame
}

fn attach_thread_stack(event: &Value, exceptions: &mut [ExceptionInfo]) {
    let Some(frames) = current_thread_frames(event) else {
        return;
    };
    if frames.is_empty() {
        return;
    }
    if let Some(exception) = exceptions
        .iter_mut()
        .rev()
        .find(|exception| exception.frames.is_empty())
    {
        exception.frames = frames;
    }
}

fn current_thread_frames(event: &Value) -> Option<Vec<FrameInfo>> {
    let threads = event
        .get("threads")
        .and_then(|threads| threads.get("values"))
        .and_then(Value::as_array)?;
    let thread = threads
        .iter()
        .find(|thread| thread.get("current").and_then(bool_of).unwrap_or(false))
        .or_else(|| threads.first())?;
    let frames = thread
        .get("stacktrace")
        .and_then(|stack| stack.get("frames"))
        .and_then(Value::as_array)?;
    Some(frames.iter().map(parse_frame).collect())
}

fn parse_breadcrumbs(event: &Value) -> Vec<BreadcrumbInfo> {
    let Some(breadcrumbs) = event.get("breadcrumbs") else {
        return Vec::new();
    };
    let values = if let Some(values) = breadcrumbs.as_array() {
        values
    } else if let Some(values) = breadcrumbs.get("values").and_then(Value::as_array) {
        values
    } else {
        return Vec::new();
    };
    values
        .iter()
        .map(|crumb| BreadcrumbInfo {
            timestamp: crumb
                .get("timestamp")
                .map(|value| timestamp_of(Some(value))),
            type_name: crumb
                .get("type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            category: crumb
                .get("category")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            level: crumb
                .get("level")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            message: crumb
                .get("message")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            data: match crumb.get("data") {
                Some(Value::Object(map)) => Value::Object(map.clone()),
                _ => Value::Object(Default::default()),
            },
        })
        .collect()
}

fn parse_tags(event: &Value) -> Vec<Tag> {
    match event.get("tags") {
        Some(Value::Object(map)) => {
            let mut tags: Vec<Tag> = map
                .iter()
                .map(|(key, value)| Tag {
                    key: key.clone(),
                    value: scalar_string(value),
                })
                .collect();
            tags.sort_by(|left, right| left.key.cmp(&right.key));
            tags
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                let pair = item.as_array()?;
                Some(Tag {
                    key: scalar_string(pair.first()?),
                    value: scalar_string(pair.get(1).unwrap_or(&Value::Null)),
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_sdk(event: &Value) -> Option<SdkInfo> {
    let sdk = event.get("sdk")?;
    let name = sdk.get("name").and_then(Value::as_str)?.to_owned();
    let version = sdk
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    Some(SdkInfo { name, version })
}

fn fingerprint_of(event: &Value, exceptions: &[ExceptionInfo], message: Option<&str>) -> String {
    let default = default_fingerprint(exceptions, message);
    let custom = event
        .get("fingerprint")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty());
    let material = if let Some(items) = custom {
        let only_default = items.len() == 1 && items[0].as_str() == Some("{{ default }}");
        if only_default {
            default
        } else {
            items
                .iter()
                .map(|item| match item.as_str() {
                    Some("{{ default }}") => default.clone(),
                    Some(value) => value.to_owned(),
                    None => scalar_string(item),
                })
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        default
    };
    hex::encode(Sha256::digest(material.as_bytes()))
}

fn default_fingerprint(exceptions: &[ExceptionInfo], message: Option<&str>) -> String {
    if exceptions.is_empty() {
        return format!("message\n{}", normalize_message(message.unwrap_or("")));
    }
    let mut parts = Vec::new();
    for exception in exceptions {
        parts.push(
            exception
                .type_name
                .clone()
                .unwrap_or_else(|| "Error".into()),
        );
        let frames = relevant_frames(&exception.frames);
        if frames.is_empty() {
            parts.push(normalize_message(exception.value.as_deref().unwrap_or("")));
        } else {
            for frame in frames {
                let function = grouping_function(frame.function.as_deref().unwrap_or("?"));
                let file = frame
                    .filename
                    .as_deref()
                    .or(frame.abs_path.as_deref())
                    .or(frame.module.as_deref())
                    .unwrap_or("?");
                parts.push(format!("{function}@{}", basename(file)));
            }
        }
    }
    parts.join("\n")
}

fn relevant_frames(frames: &[FrameInfo]) -> Vec<&FrameInfo> {
    let usable: Vec<&FrameInfo> = frames
        .iter()
        .filter(|frame| !is_runtime_frame(frame) && frame_has_identity(frame))
        .collect();
    let in_app: Vec<&FrameInfo> = usable
        .iter()
        .copied()
        .filter(|frame| frame.in_app)
        .collect();
    if in_app.is_empty() { usable } else { in_app }
}

fn grouping_function(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut rest = name;
    while let Some(index) = rest.find("closure#") {
        output.push_str(&rest[..index]);
        output.push_str("closure");
        rest = &rest[index + "closure#".len()..];
        let digits = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
        rest = &rest[digits..];
    }
    output.push_str(rest);
    output
}

fn frame_has_identity(frame: &FrameInfo) -> bool {
    frame.function.is_some()
        || frame.filename.is_some()
        || frame.abs_path.is_some()
        || frame.module.is_some()
}

/// SDK and runtime frames are not part of the application stack. The Rust SDK
/// often marks its own crates `in_app`, and the symbol hash changes between builds.
fn is_runtime_frame(frame: &FrameInfo) -> bool {
    let function = frame.function.as_deref().unwrap_or("");
    if function.is_empty() || function == "<unknown>" || function == "?" {
        return frame.filename.is_none() && frame.abs_path.is_none() && frame.module.is_none();
    }
    let file = frame.filename.as_deref().unwrap_or("");
    let path = frame.abs_path.as_deref().unwrap_or("");
    let module = frame.module.as_deref().unwrap_or("");
    let blob = format!("{function} {file} {path} {module}").to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "sentry_backtrace",
        "sentry_core",
        "sentry_panic",
        "sentry_contexts",
        "sentry::",
        "sentry-",
        "/sentry/",
        "@sentry/",
        "sentrywrapped",
        "backtrace::",
        "std::panicking",
        "std::sys::backtrace",
        "std::backtrace",
        "std::rt::lang_start",
        "rust_begin_short_backtrace",
        "__rust_try",
        "core::ops::function::fnonce",
        "core::panic::",
        "current_stacktrace",
    ];
    NEEDLES.iter().any(|needle| blob.contains(needle))
}

fn normalize_message(message: &str) -> String {
    let mut output = String::new();
    let mut previous_space = false;
    for ch in message.chars() {
        let mapped = if ch.is_ascii_digit() { '0' } else { ch };
        if mapped.is_whitespace() {
            if !previous_space && !output.is_empty() {
                output.push(' ');
            }
            previous_space = true;
        } else {
            output.push(mapped);
            previous_space = false;
        }
    }
    truncate(output.trim(), 200)
}

fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let mut output: String = value.chars().take(max.saturating_sub(1)).collect();
    output.push('…');
    output
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn scalar_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn u32_of(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| {
            value
                .as_i64()
                .filter(|number| *number >= 0)
                .and_then(|number| u32::try_from(number).ok())
        })
        .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
}

fn bool_of(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| match value.as_i64() {
        Some(0) => Some(false),
        Some(1) => Some(true),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "event_id": "ABC-def",
            "timestamp": 1_700_000_000.5,
            "level": "error",
            "platform": "php",
            "environment": "production",
            "release": "php@1",
            "sdk": {"name": "sentry.php", "version": "4.10.0"},
            "exception": {
                "values": [{
                    "type": "RuntimeException",
                    "value": "disk full on volume 12",
                    "mechanism": {"type": "generic", "handled": false},
                    "stacktrace": {
                        "frames": [
                            {"filename": "/vendor/lib.php", "function": "boot", "lineno": 10, "in_app": false},
                            {
                                "filename": "/app/src/Store.php",
                                "function": "save",
                                "lineno": 42,
                                "in_app": true,
                                "context_line": "throw new RuntimeException('disk full');",
                                "pre_context": ["$disk->check();"],
                                "post_context": ["}"]
                            }
                        ]
                    }
                }]
            },
            "breadcrumbs": {"values": [{"category": "query", "message": "insert", "level": "info"}]},
            "tags": {"runtime": "php"},
            "contexts": {"runtime": {"name": "php", "version": "8.3"}},
            "user": {"id": "7"},
            "request": {"method": "POST", "url": "http://app/checkout"}
        })
    }

    #[test]
    fn normalizes_exception_stack_and_groups_without_numbers() {
        let event = normalize_event(sample());
        assert_eq!(event.event_id, "abcdef");
        assert!(event.timestamp.starts_with("2023-11-14"));
        assert_eq!(event.exceptions.len(), 1);
        assert_eq!(
            event.exceptions[0].type_name.as_deref(),
            Some("RuntimeException")
        );
        assert_eq!(event.exceptions[0].handled, Some(false));
        assert_eq!(event.exceptions[0].frames.len(), 2);
        assert_eq!(event.exceptions[0].frames[1].pre_context.len(), 1);
        assert_eq!(event.sdk.as_ref().unwrap().name, "sentry.php");
        assert_eq!(event.tags[0].key, "runtime");
        assert_eq!(event.culprit.as_deref(), Some("save"));
        assert!(event.title.starts_with("RuntimeException:"));

        let mut other = sample();
        other["exception"]["values"][0]["value"] = serde_json::json!("disk full on volume 99");
        other["event_id"] = serde_json::json!("ffff");
        let other = normalize_event(other);
        assert_eq!(event.fingerprint, other.fingerprint);
    }

    #[test]
    fn uses_thread_stack_when_exception_has_no_frames() {
        let event = normalize_event(serde_json::json!({
            "exception": {"values": [{"type": "ExampleError", "value": "manual"}]},
            "threads": {"values": [{
                "current": true,
                "stacktrace": {"frames": [
                    {"filename": "main.rs", "function": "checkout::main", "lineno": 12, "in_app": true}
                ]}
            }]}
        }));
        assert_eq!(event.exceptions[0].frames.len(), 1);
        assert_eq!(
            event.exceptions[0].frames[0].function.as_deref(),
            Some("checkout::main")
        );
        assert_eq!(event.culprit.as_deref(), Some("checkout::main"));
    }

    #[test]
    fn rust_grouping_ignores_sdk_frames_and_thread_attachment() {
        let app = || {
            serde_json::json!({
                "filename": "main.rs",
                "abs_path": "/workspace/examples/rust/src/main.rs",
                "function": "haruvo_rust_example::main",
                "lineno": 40,
                "in_app": true
            })
        };
        let sentry = |hash: &str| {
            serde_json::json!({
                "function": format!("sentry_backtrace[{hash}]::current_stacktrace"),
                "filename": "backtrace.rs",
                "in_app": true
            })
        };
        let with_stack = normalize_event(serde_json::json!({
            "exception": {"values": [{
                "type": "ExampleError",
                "value": "manual error from the Rust example",
                "stacktrace": {"frames": [app(), sentry("aaa111")]}
            }]}
        }));
        let thread_only = normalize_event(serde_json::json!({
            "exception": {"values": [{
                "type": "ExampleError",
                "value": "manual error from the Rust example"
            }]},
            "threads": {"values": [{
                "current": true,
                "stacktrace": {"frames": [app(), sentry("bbb222")]}
            }]}
        }));
        assert_eq!(with_stack.fingerprint, thread_only.fingerprint);
        assert_eq!(
            with_stack.culprit.as_deref(),
            Some("haruvo_rust_example::main")
        );
        assert_eq!(
            thread_only.culprit.as_deref(),
            Some("haruvo_rust_example::main")
        );
        assert!(
            with_stack.exceptions[0]
                .frames
                .iter()
                .any(
                    |frame| frame.function.as_deref() == Some("haruvo_rust_example::main")
                        && frame.in_app
                )
        );
        assert!(with_stack.exceptions[0].frames.iter().any(|frame| {
            !frame.in_app
                && frame
                    .function
                    .as_deref()
                    .unwrap_or("")
                    .contains("sentry_backtrace")
        }));
        let panic_frame = |closure: &str| {
            serde_json::json!({
                "filename": "main.rs",
                "function": format!("haruvo_rust_example::main::{{closure#{closure}}}"),
                "lineno": 43,
                "in_app": true
            })
        };
        let panic_a = normalize_event(serde_json::json!({
            "exception": {"values": [{"type": "panic", "value": "panic from the Rust example", "stacktrace": {"frames": [panic_frame("1")]}}]}
        }));
        let panic_b = normalize_event(serde_json::json!({
            "exception": {"values": [{"type": "panic", "value": "panic from the Rust example", "stacktrace": {"frames": [panic_frame("2")]}}]}
        }));
        assert_eq!(panic_a.fingerprint, panic_b.fingerprint);
    }

    #[test]
    fn honors_sdk_fingerprint() {
        let mut raw = sample();
        raw["fingerprint"] = serde_json::json!(["custom-group"]);
        let event = normalize_event(raw);
        let mut other = sample();
        other["fingerprint"] = serde_json::json!(["custom-group"]);
        other["exception"]["values"][0]["type"] = serde_json::json!("Other");
        let other = normalize_event(other);
        assert_eq!(event.fingerprint, other.fingerprint);

        let mut plain = sample();
        plain["fingerprint"] = serde_json::json!(["{{ default }}"]);
        let plain = normalize_event(plain);
        let original = normalize_event(sample());
        assert_eq!(plain.fingerprint, original.fingerprint);
    }
}
