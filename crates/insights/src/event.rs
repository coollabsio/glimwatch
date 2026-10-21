use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::url::clip;

pub const TYPE_STRING: i64 = 1;
pub const TYPE_NUMBER: i64 = 2;
pub const TYPE_BOOLEAN: i64 = 3;
pub const TYPE_ARRAY: i64 = 5;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Vitals {
    pub lcp: Option<f64>,
    pub inp: Option<f64>,
    pub cls: Option<f64>,
    pub fcp: Option<f64>,
    pub ttfb: Option<f64>,
}

#[derive(Clone, Debug)]
pub struct DataValue {
    pub key: String,
    pub string_value: String,
    pub number_value: Option<f64>,
    pub data_type: i64,
}

#[derive(Clone, Debug)]
pub struct Incoming {
    pub site_id: Option<String>,
    pub link_id: Option<String>,
    pub timestamp: i64,
    pub event_type: String,
    pub session_id: Option<String>,
    pub pathname: Option<String>,
    pub url_query: Option<String>,
    pub title: Option<String>,
    pub hostname: Option<String>,
    pub utm: [Option<String>; 5],
    pub referrer_domain: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub browser: Option<String>,
    pub os: Option<String>,
    pub device: Option<String>,
    pub screen: Option<String>,
    pub language: Option<String>,
    pub metadata: Option<String>,
    pub vitals: Vitals,
    pub data: Vec<DataValue>,
}

impl Default for Incoming {
    fn default() -> Self {
        Self {
            site_id: None,
            link_id: None,
            timestamp: 0,
            event_type: String::new(),
            session_id: None,
            pathname: None,
            url_query: None,
            title: None,
            hostname: None,
            utm: Default::default(),
            referrer_domain: None,
            country: None,
            region: None,
            city: None,
            browser: None,
            os: None,
            device: None,
            screen: None,
            language: None,
            metadata: None,
            vitals: Vitals::default(),
            data: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct EventPayload {
    pub site: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub referrer: Option<String>,
    pub title: Option<String>,
    pub hostname: Option<String>,
    pub screen: Option<String>,
    pub language: Option<String>,
    pub vitals: Option<Vitals>,
    pub data: Option<Value>,
}

pub fn clean_vitals(vitals: Vitals) -> Vitals {
    let time = |value: Option<f64>| {
        value.filter(|value| value.is_finite() && (0.0..=60_000.0).contains(value))
    };
    Vitals {
        lcp: time(vitals.lcp),
        inp: time(vitals.inp),
        cls: vitals
            .cls
            .filter(|value| value.is_finite() && (0.0..=100.0).contains(value)),
        fcp: time(vitals.fcp),
        ttfb: time(vitals.ttfb),
    }
}

const MAX_KEYS: usize = 50;
const MAX_LEN: usize = 500;

pub fn flatten(value: &Value) -> Vec<DataValue> {
    let mut out = Vec::new();
    if let Value::Object(map) = value {
        walk(map, "", &mut out);
    }
    out
}

fn walk(map: &serde_json::Map<String, Value>, prefix: &str, out: &mut Vec<DataValue>) {
    for (key, value) in map {
        if out.len() >= MAX_KEYS {
            return;
        }
        let key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let (string_value, number_value, data_type) = match value {
            Value::Object(inner) => {
                walk(inner, &key, out);
                continue;
            }
            Value::Null => continue,
            Value::Bool(flag) => (flag.to_string(), None, TYPE_BOOLEAN),
            Value::Number(number) => (number.to_string(), number.as_f64(), TYPE_NUMBER),
            Value::String(text) => (text.clone(), None, TYPE_STRING),
            Value::Array(_) => {
                let json = value.to_string();
                if json.len() > MAX_LEN {
                    continue;
                }
                (json, None, TYPE_ARRAY)
            }
        };
        out.push(DataValue {
            key: clip(&key, MAX_LEN),
            string_value: clip(&string_value, MAX_LEN),
            number_value,
            data_type,
        });
    }
}
