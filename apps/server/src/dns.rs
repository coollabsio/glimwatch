//! TXT lookup for domain verification.
//!
//! Production uses the system resolver. Tests inject [`MapResolver`] so the
//! suite never sends a packet.

use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait TxtResolver: Send + Sync {
    fn lookup_txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, String>>;
}

#[derive(Clone, Default)]
pub struct MapResolver {
    records: Arc<Mutex<HashMap<String, Vec<String>>>>,
}

impl MapResolver {
    pub fn set(&self, name: impl Into<String>, values: Vec<String>) {
        self.records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .insert(name.into().trim_end_matches('.').to_ascii_lowercase(), values);
    }
}

impl TxtResolver for MapResolver {
    fn lookup_txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, String>> {
        let key = name.trim_end_matches('.').to_ascii_lowercase();
        let found = self
            .records
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&key)
            .cloned()
            .unwrap_or_default();
        Box::pin(async move { Ok(found) })
    }
}

/// UDP TXT lookup against the first `nameserver` in `/etc/resolv.conf`.
pub struct SystemResolver {
    server: String,
}

impl SystemResolver {
    pub fn from_system() -> Self {
        let server = std::fs::read_to_string("/etc/resolv.conf")
            .ok()
            .and_then(|text| {
                text.lines().find_map(|line| {
                    let line = line.trim();
                    let rest = line.strip_prefix("nameserver ")?;
                    let ip = rest.split_whitespace().next()?.trim();
                    (!ip.is_empty()).then(|| ip.to_owned())
                })
            })
            .unwrap_or_else(|| "1.1.1.1".to_owned());
        Self { server }
    }
}

impl TxtResolver for SystemResolver {
    fn lookup_txt<'a>(&'a self, name: &'a str) -> BoxFuture<'a, Result<Vec<String>, String>> {
        let server = self.server.clone();
        let name = name.to_owned();
        Box::pin(async move { lookup_txt(&server, &name).await })
    }
}

async fn lookup_txt(server: &str, name: &str) -> Result<Vec<String>, String> {
    let id = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(1)
        % 65535) as u16;
    let query = encode_query(name, id).map_err(|error| error.to_owned())?;
    let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|error| error.to_string())?;
    let address: SocketAddr = format!("{server}:53")
        .parse()
        .map_err(|_| format!("nameserver {server} is not an IP address"))?;
    socket
        .send_to(&query, address)
        .await
        .map_err(|error| error.to_string())?;
    let mut buffer = [0_u8; 2048];
    let (length, _) = tokio::time::timeout(Duration::from_secs(3), socket.recv_from(&mut buffer))
        .await
        .map_err(|_| "DNS lookup timed out".to_owned())?
        .map_err(|error| error.to_string())?;
    parse_txt_response(&buffer[..length], id)
}

fn encode_query(name: &str, id: u16) -> Result<Vec<u8>, &'static str> {
    let mut body = Vec::new();
    body.extend_from_slice(&id.to_be_bytes());
    body.extend_from_slice(&0x0100_u16.to_be_bytes());
    body.extend_from_slice(&1_u16.to_be_bytes());
    body.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err("invalid DNS name");
        }
        body.push(label.len() as u8);
        body.extend_from_slice(label.as_bytes());
    }
    body.push(0);
    body.extend_from_slice(&16_u16.to_be_bytes());
    body.extend_from_slice(&1_u16.to_be_bytes());
    Ok(body)
}

pub fn parse_txt_response(buffer: &[u8], id: u16) -> Result<Vec<String>, String> {
    if buffer.len() < 12 {
        return Err("short DNS response".into());
    }
    let response_id = u16::from_be_bytes([buffer[0], buffer[1]]);
    if response_id != id {
        return Err("DNS response id mismatch".into());
    }
    let flags = u16::from_be_bytes([buffer[2], buffer[3]]);
    let rcode = flags & 0x000f;
    if rcode == 3 {
        return Ok(Vec::new());
    }
    if rcode != 0 {
        return Err(format!("DNS rcode {rcode}"));
    }
    let questions = u16::from_be_bytes([buffer[4], buffer[5]]) as usize;
    let answers = u16::from_be_bytes([buffer[6], buffer[7]]) as usize;
    let mut index = 12;
    for _ in 0..questions {
        index = skip_name(buffer, index)?;
        index = index.checked_add(4).ok_or("truncated DNS question")?;
        if index > buffer.len() {
            return Err("truncated DNS question".into());
        }
    }
    let mut records = Vec::new();
    for _ in 0..answers {
        index = skip_name(buffer, index)?;
        if index + 10 > buffer.len() {
            return Err("truncated DNS answer".into());
        }
        let kind = u16::from_be_bytes([buffer[index], buffer[index + 1]]);
        let rdlen = u16::from_be_bytes([buffer[index + 8], buffer[index + 9]]) as usize;
        index += 10;
        let end = index.checked_add(rdlen).ok_or("truncated DNS rdata")?;
        if end > buffer.len() {
            return Err("truncated DNS rdata".into());
        }
        if kind == 16 {
            records.push(decode_txt(&buffer[index..end])?);
        }
        index = end;
    }
    Ok(records)
}

fn decode_txt(rdata: &[u8]) -> Result<String, String> {
    let mut text = String::new();
    let mut index = 0;
    while index < rdata.len() {
        let length = rdata[index] as usize;
        index += 1;
        let end = index.checked_add(length).ok_or("truncated TXT")?;
        if end > rdata.len() {
            return Err("truncated TXT".into());
        }
        text.push_str(&String::from_utf8_lossy(&rdata[index..end]));
        index = end;
    }
    Ok(text)
}

fn skip_name(buffer: &[u8], mut index: usize) -> Result<usize, String> {
    let mut hops = 0;
    loop {
        if index >= buffer.len() {
            return Err("truncated DNS name".into());
        }
        let length = buffer[index];
        if length == 0 {
            return Ok(index + 1);
        }
        if length & 0xc0 == 0xc0 {
            if index + 1 >= buffer.len() {
                return Err("truncated DNS pointer".into());
            }
            return Ok(index + 2);
        }
        hops += 1;
        if hops > 32 {
            return Err("DNS name is too long".into());
        }
        index += 1 + length as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_txt_answer() {
        let query = encode_query("_owlee-verify.example.com", 0x1234).unwrap();
        let mut response = query.clone();
        response[2] = 0x81;
        response[3] = 0x80;
        response[6] = 0;
        response[7] = 1;
        response.extend_from_slice(&[0xc0, 0x0c]);
        response.extend_from_slice(&16_u16.to_be_bytes());
        response.extend_from_slice(&1_u16.to_be_bytes());
        response.extend_from_slice(&60_u32.to_be_bytes());
        let text = b"owlee-verify=abc";
        response.extend_from_slice(&(text.len() as u16 + 1).to_be_bytes());
        response.push(text.len() as u8);
        response.extend_from_slice(text);
        let records = parse_txt_response(&response, 0x1234).unwrap();
        assert_eq!(records, vec!["owlee-verify=abc".to_owned()]);
    }

    #[test]
    fn nxdomain_is_an_empty_answer() {
        let mut response = encode_query("example.com", 7).unwrap();
        response[2] = 0x81;
        response[3] = 0x83;
        let records = parse_txt_response(&response, 7).unwrap();
        assert!(records.is_empty());
    }
}
