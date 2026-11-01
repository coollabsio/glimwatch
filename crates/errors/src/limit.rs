//! Caps stored Sentry payloads so one event cannot fill the database.
//!
//! Fingerprinting runs on the original event first. This pass then clips the
//! JSON that is written to `events.payload`.

use serde_json::{Map, Value};

const MAX_MESSAGE: usize = 8 * 1024;
const MAX_FRAMES: usize = 50;
const MAX_BREADCRUMBS: usize = 50;
const MAX_TAGS: usize = 30;
const MAX_TAG_KEY: usize = 32;
const MAX_TAG_VALUE: usize = 200;
const MAX_LINE: usize = 500;
const MAX_CONTEXT_LINES: usize = 5;
const MAX_STRING: usize = 8 * 1024;
const MAX_ARRAY: usize = 100;
const MAX_KEYS: usize = 40;
const MAX_DEPTH: usize = 8;

pub fn limit_raw_event(raw: &mut Value) {
    clip_message(raw.get_mut("message"));
    clip_message(raw.get_mut("logentry"));
    if let Some(exception) = raw.get_mut("exception") {
        clip_exception(exception);
    }
    if let Some(threads) = raw.get_mut("threads") {
        clip_threads(threads);
    }
    clip_breadcrumbs(raw.get_mut("breadcrumbs"));
    clip_tags(raw.get_mut("tags"));
    clip_value(raw, 0);
}

fn clip_exception(exception: &mut Value) {
    if let Some(values) = exception.get_mut("values").and_then(Value::as_array_mut) {
        for value in values {
            clip_field(value, "value", MAX_MESSAGE);
            clip_frames(value.get_mut("stacktrace"));
        }
        return;
    }
    clip_field(exception, "value", MAX_MESSAGE);
    clip_frames(exception.get_mut("stacktrace"));
}

fn clip_threads(threads: &mut Value) {
    let Some(values) = threads.get_mut("values").and_then(Value::as_array_mut) else {
        return;
    };
    for thread in values {
        clip_frames(thread.get_mut("stacktrace"));
    }
}

fn clip_frames(stack: Option<&mut Value>) {
    let Some(frames) = stack
        .and_then(|stack| stack.get_mut("frames"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    if frames.len() > MAX_FRAMES {
        let drop = frames.len() - MAX_FRAMES;
        frames.drain(0..drop);
    }
    for frame in frames {
        clip_field(frame, "filename", MAX_LINE);
        clip_field(frame, "abs_path", MAX_LINE);
        clip_field(frame, "function", 300);
        clip_field(frame, "module", 300);
        clip_field(frame, "context_line", MAX_LINE);
        clip_string_array(frame, "pre_context");
        clip_string_array(frame, "post_context");
    }
}

fn clip_string_array(frame: &mut Value, key: &str) {
    let Some(lines) = frame.get_mut(key).and_then(Value::as_array_mut) else {
        return;
    };
    if lines.len() > MAX_CONTEXT_LINES {
        lines.truncate(MAX_CONTEXT_LINES);
    }
    for line in lines {
        if let Some(text) = line.as_str() {
            let clipped = clip_str(text, 300);
            if clipped != text {
                *line = Value::String(clipped);
            }
        }
    }
}

fn clip_breadcrumbs(breadcrumbs: Option<&mut Value>) {
    let Some(breadcrumbs) = breadcrumbs else {
        return;
    };
    let values = if breadcrumbs.is_array() {
        breadcrumbs.as_array_mut()
    } else {
        breadcrumbs.get_mut("values").and_then(Value::as_array_mut)
    };
    let Some(values) = values else {
        return;
    };
    if values.len() > MAX_BREADCRUMBS {
        let drop = values.len() - MAX_BREADCRUMBS;
        values.drain(0..drop);
    }
    for crumb in values {
        clip_field(crumb, "message", 1024);
        clip_field(crumb, "category", 200);
        if let Some(data) = crumb.get_mut("data") {
            clip_value(data, 0);
        }
    }
}

fn clip_tags(tags: Option<&mut Value>) {
    let Some(tags) = tags else {
        return;
    };
    match tags {
        Value::Object(map) => {
            if map.len() > MAX_TAGS {
                let mut keys: Vec<String> = map.keys().cloned().collect();
                keys.sort();
                for key in keys.into_iter().skip(MAX_TAGS) {
                    map.remove(&key);
                }
            }
            let keys: Vec<String> = map.keys().cloned().collect();
            for key in keys {
                let Some(value) = map.remove(&key) else {
                    continue;
                };
                let key = clip_str(&key, MAX_TAG_KEY);
                let value = match value {
                    Value::String(text) => Value::String(clip_str(&text, MAX_TAG_VALUE)),
                    other => Value::String(clip_str(&scalar(&other), MAX_TAG_VALUE)),
                };
                map.insert(key, value);
            }
        }
        Value::Array(items) => {
            if items.len() > MAX_TAGS {
                items.truncate(MAX_TAGS);
            }
            for item in items {
                clip_field(item, "key", MAX_TAG_KEY);
                clip_field(item, "value", MAX_TAG_VALUE);
                if let Some(pair) = item.as_array_mut() {
                    if let Some(key) = pair.first().and_then(Value::as_str) {
                        let clipped = clip_str(key, MAX_TAG_KEY);
                        if let Some(slot) = pair.first_mut() {
                            *slot = Value::String(clipped);
                        }
                    }
                    if pair.len() > 1 {
                        if let Some(value) = pair.get(1).map(scalar) {
                            let clipped = clip_str(&value, MAX_TAG_VALUE);
                            if let Some(slot) = pair.get_mut(1) {
                                *slot = Value::String(clipped);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn clip_message(value: Option<&mut Value>) {
    let Some(value) = value else {
        return;
    };
    match value {
        Value::String(text) => {
            let clipped = clip_str(text, MAX_MESSAGE);
            if clipped != *text {
                *text = clipped;
            }
        }
        Value::Object(map) => {
            for key in ["formatted", "message"] {
                if let Some(Value::String(text)) = map.get_mut(key) {
                    let clipped = clip_str(text, MAX_MESSAGE);
                    if clipped != *text {
                        *text = clipped;
                    }
                }
            }
        }
        _ => {}
    }
}

fn clip_field(value: &mut Value, key: &str, max: usize) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(current) = object.get(key).and_then(Value::as_str).map(str::to_owned) else {
        return;
    };
    let clipped = clip_str(&current, max);
    if clipped != current {
        object.insert(key.to_owned(), Value::String(clipped));
    }
}

fn clip_value(value: &mut Value, depth: usize) {
    if depth > MAX_DEPTH {
        *value = Value::Null;
        return;
    }
    match value {
        Value::String(text) => {
            let clipped = clip_str(text, MAX_STRING);
            if clipped != *text {
                *text = clipped;
            }
        }
        Value::Array(items) => {
            if items.len() > MAX_ARRAY {
                items.truncate(MAX_ARRAY);
            }
            for item in items {
                clip_value(item, depth + 1);
            }
        }
        Value::Object(map) => clip_map(map, depth),
        _ => {}
    }
}

fn clip_map(map: &mut Map<String, Value>, depth: usize) {
    if map.len() > MAX_KEYS {
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        for key in keys.into_iter().skip(MAX_KEYS) {
            map.remove(&key);
        }
    }
    let keys: Vec<String> = map.keys().cloned().collect();
    for key in keys {
        if let Some(child) = map.get_mut(&key) {
            clip_value(child, depth + 1);
        }
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn clip_str(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let mut output: String = value.chars().take(max.saturating_sub(1)).collect();
    output.push('…');
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn truncates_stacks_breadcrumbs_tags_and_messages() {
        let mut frames = Vec::new();
        for index in 0..80 {
            frames.push(json!({
                "filename": format!("/app/file{index}.rs"),
                "function": "run",
                "context_line": "x".repeat(800),
                "pre_context": ["y".repeat(400), "z", "a", "b", "c", "d", "e"]
            }));
        }
        let mut tags = Map::new();
        for index in 0..40 {
            tags.insert(format!("key{index:02}"), Value::String("v".repeat(300)));
        }
        let mut raw = json!({
            "message": "m".repeat(20_000),
            "exception": {"values": [{"type": "Panic", "value": "e".repeat(20_000), "stacktrace": {"frames": frames}}]},
            "breadcrumbs": {"values": (0..80).map(|index| json!({"message": format!("crumb-{index}-{}", "b".repeat(2000))})).collect::<Vec<_>>()},
            "tags": tags,
        });
        limit_raw_event(&mut raw);
        let message = raw["message"].as_str().unwrap();
        assert!(message.chars().count() <= MAX_MESSAGE, "{}", message.chars().count());
        assert!(message.ends_with('…'));
        let frames = raw["exception"]["values"][0]["stacktrace"]["frames"]
            .as_array()
            .unwrap();
        assert_eq!(frames.len(), MAX_FRAMES);
        assert!(frames[0]["filename"].as_str().unwrap().ends_with("file30.rs"));
        assert!(frames[0]["context_line"].as_str().unwrap().chars().count() <= MAX_LINE);
        assert_eq!(frames[0]["pre_context"].as_array().unwrap().len(), MAX_CONTEXT_LINES);
        assert_eq!(raw["breadcrumbs"]["values"].as_array().unwrap().len(), MAX_BREADCRUMBS);
        assert!(
            raw["breadcrumbs"]["values"][0]["message"]
                .as_str()
                .unwrap()
                .chars()
                .count()
                <= 1024
        );
        assert!(raw["tags"].as_object().unwrap().len() <= MAX_TAGS);
        let long = raw["tags"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .and_then(Value::as_str)
            .unwrap();
        assert!(long.chars().count() <= MAX_TAG_VALUE);
        let exception = raw["exception"]["values"][0]["value"].as_str().unwrap();
        assert!(exception.chars().count() <= MAX_MESSAGE);
    }
}
