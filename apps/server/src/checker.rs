use std::time::{Duration, Instant};

use sqlx::Connection;
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::postgres::{PgConnectOptions, PgConnection};
use status_domain::{HeaderPair, JobType, Probe, parse_tcp_target};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use crate::repo::Monitor;
use crate::state::HttpClients;

const BODY_LIMIT: usize = 65_536;

/// What a runner needs to probe an HTTP or TCP target.
#[derive(Clone, Debug)]
pub struct Job {
    pub job_type: JobType,
    pub url: String,
    pub method: String,
    pub headers: Vec<HeaderPair>,
    pub body: String,
    pub timeout_ms: i64,
    pub follow_redirects: bool,
}

impl From<&Monitor> for Job {
    fn from(monitor: &Monitor) -> Self {
        Self {
            job_type: monitor.job_type,
            url: monitor.url.clone(),
            method: monitor.method.clone(),
            headers: monitor.headers.clone(),
            body: monitor.body.clone(),
            timeout_ms: monitor.timeout_ms,
            follow_redirects: monitor.follow_redirects,
        }
    }
}

pub async fn probe(http: &HttpClients, job: &Job) -> Probe {
    let started = Instant::now();
    let timeout = Duration::from_millis(job.timeout_ms.max(1) as u64);
    match job.job_type {
        JobType::Http => http_probe(http, job, timeout, started).await,
        JobType::Tcp => tcp_probe(job, timeout, started).await,
    }
}

async fn http_probe(
    http: &HttpClients,
    job: &Job,
    timeout: Duration,
    started: Instant,
) -> Probe {
    let client = if job.follow_redirects {
        &http.follow
    } else {
        &http.direct
    };
    let method =
        reqwest::Method::from_bytes(job.method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut request = client
        .request(method.clone(), &job.url)
        .timeout(timeout);
    let mut headers = reqwest::header::HeaderMap::new();
    for header in &job.headers {
        let Ok(name) = reqwest::header::HeaderName::from_bytes(header.name.as_bytes()) else {
            continue;
        };
        let Ok(value) = reqwest::header::HeaderValue::from_str(&header.value) else {
            continue;
        };
        headers.insert(name, value);
    }
    request = request.headers(headers);
    if matches!(
        method,
        reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH
    ) && !job.body.is_empty()
    {
        request = request.body(job.body.clone());
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return Probe {
                latency_ms: elapsed_ms(started),
                transport_error: Some(trim_error(&error.to_string())),
                ..Probe::default()
            };
        }
    };
    let status_code = i64::from(response.status().as_u16());
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut body = Vec::new();
    let mut response = response;
    loop {
        if body.len() >= BODY_LIMIT {
            break;
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let room = BODY_LIMIT - body.len();
                let take = chunk.len().min(room);
                body.extend_from_slice(&chunk[..take]);
            }
            Ok(None) => break,
            Err(error) => {
                return Probe {
                    status_code: Some(status_code),
                    headers,
                    latency_ms: elapsed_ms(started),
                    transport_error: Some(trim_error(&error.to_string())),
                    ..Probe::default()
                };
            }
        }
    }
    Probe {
        status_code: Some(status_code),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
        latency_ms: elapsed_ms(started),
        transport_error: None,
    }
}

async fn tcp_probe(job: &Job, timeout: Duration, started: Instant) -> Probe {
    let Some((host, port)) = parse_tcp_target(&job.url) else {
        return Probe {
            latency_ms: 0,
            transport_error: Some("TCP target must look like host:port.".to_owned()),
            ..Probe::default()
        };
    };
    match tokio::time::timeout(timeout, TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(stream)) => {
            drop(stream);
            Probe {
                latency_ms: elapsed_ms(started),
                ..Probe::default()
            }
        }
        Ok(Err(error)) => Probe {
            latency_ms: elapsed_ms(started),
            transport_error: Some(trim_error(&error.to_string())),
            ..Probe::default()
        },
        Err(_) => Probe {
            latency_ms: elapsed_ms(started),
            transport_error: Some(format!("timed out after {}ms", timeout.as_millis())),
            ..Probe::default()
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Database {
    Postgres,
    Mysql,
    Redis,
}

/// Credentials for a database probe. They exist only on the agent host.
#[derive(Clone, Debug, Default)]
pub struct Login {
    pub user: Option<String>,
    pub password: Option<String>,
    pub database: Option<String>,
}

/// Logs in and runs a trivial command (`SELECT 1` or `PING`), so a pass proves more than an
/// open port.
pub async fn database_probe(kind: Database, target: &str, login: &Login, timeout: Duration) -> Probe {
    let started = Instant::now();
    let Some((host, port)) = parse_tcp_target(target) else {
        return Probe {
            transport_error: Some("Target must look like host:port.".to_owned()),
            ..Probe::default()
        };
    };
    let attempt = async {
        match kind {
            Database::Postgres => postgres(&host, port, login).await,
            Database::Mysql => mysql(&host, port, login).await,
            Database::Redis => redis(&host, port, login).await,
        }
    };
    let transport_error = match tokio::time::timeout(timeout, attempt).await {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(trim_error(&error)),
        Err(_) => Some(format!("timed out after {}ms", timeout.as_millis())),
    };
    Probe {
        latency_ms: elapsed_ms(started),
        transport_error,
        ..Probe::default()
    }
}

async fn postgres(host: &str, port: u16, login: &Login) -> Result<(), String> {
    let mut options = PgConnectOptions::new().host(host).port(port);
    if let Some(user) = &login.user {
        options = options.username(user);
    }
    if let Some(password) = &login.password {
        options = options.password(password);
    }
    if let Some(database) = &login.database {
        options = options.database(database);
    }
    let mut connection = PgConnection::connect_with(&options)
        .await
        .map_err(|error| error.to_string())?;
    let result = sqlx::query("SELECT 1").execute(&mut connection).await;
    let _ = connection.close().await;
    result.map(|_| ()).map_err(|error| error.to_string())
}

async fn mysql(host: &str, port: u16, login: &Login) -> Result<(), String> {
    let mut options = MySqlConnectOptions::new().host(host).port(port);
    if let Some(user) = &login.user {
        options = options.username(user);
    }
    if let Some(password) = &login.password {
        options = options.password(password);
    }
    if let Some(database) = &login.database {
        options = options.database(database);
    }
    let mut connection = MySqlConnection::connect_with(&options)
        .await
        .map_err(|error| error.to_string())?;
    let result = sqlx::query("SELECT 1").execute(&mut connection).await;
    let _ = connection.close().await;
    result.map(|_| ()).map_err(|error| error.to_string())
}

async fn redis(host: &str, port: u16, login: &Login) -> Result<(), String> {
    let stream = TcpStream::connect((host, port))
        .await
        .map_err(|error| error.to_string())?;
    let mut stream = BufReader::new(stream);
    if let Some(password) = &login.password {
        let auth = match &login.user {
            Some(user) => vec!["AUTH", user.as_str(), password.as_str()],
            None => vec!["AUTH", password.as_str()],
        };
        redis_command(&mut stream, &auth, "+OK").await?;
    }
    redis_command(&mut stream, &["PING"], "+PONG").await
}

async fn redis_command(
    stream: &mut BufReader<TcpStream>,
    args: &[&str],
    expected: &str,
) -> Result<(), String> {
    let mut request = format!("*{}\r\n", args.len());
    for arg in args {
        request.push_str(&format!("${}\r\n{arg}\r\n", arg.len()));
    }
    stream
        .get_mut()
        .write_all(request.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    let mut reply = String::new();
    stream
        .read_line(&mut reply)
        .await
        .map_err(|error| error.to_string())?;
    let reply = reply.trim_end();
    if reply == expected {
        Ok(())
    } else if let Some(error) = reply.strip_prefix('-') {
        Err(error.to_owned())
    } else {
        Err(format!("unexpected reply: {reply}"))
    }
}

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn trim_error(error: &str) -> String {
    let mut chars = error.chars();
    let trimmed = chars.by_ref().take(500).collect::<String>();
    if chars.next().is_some() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    async fn fake_redis(replies: &'static [&'static str]) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0; 512];
            for reply in replies {
                let _ = socket.read(&mut buffer).await.unwrap();
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        address
    }

    #[tokio::test]
    async fn redis_passes_on_pong_after_auth() {
        let target = fake_redis(&["+OK\r\n", "+PONG\r\n"]).await;
        let login = Login {
            password: Some("secret".to_owned()),
            ..Login::default()
        };
        let probe = database_probe(Database::Redis, &target, &login, Duration::from_secs(2)).await;
        assert_eq!(probe.transport_error, None);
    }

    #[tokio::test]
    async fn redis_reports_the_server_error() {
        let target = fake_redis(&["-WRONGPASS invalid username-password pair\r\n"]).await;
        let login = Login {
            password: Some("bad".to_owned()),
            ..Login::default()
        };
        let probe = database_probe(Database::Redis, &target, &login, Duration::from_secs(2)).await;
        assert_eq!(
            probe.transport_error.as_deref(),
            Some("WRONGPASS invalid username-password pair")
        );
    }
}
