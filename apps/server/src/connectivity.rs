//! Tells a down target apart from a Status host that has lost its network.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

const TIMEOUT: Duration = Duration::from_secs(3);
const CACHE: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct Connectivity {
    targets: Arc<[String]>,
    last: Arc<Mutex<Option<(Instant, bool)>>>,
}

impl Connectivity {
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets: targets.into(),
            last: Arc::default(),
        }
    }

    /// True when any target accepts a TCP connection, or when no targets are
    /// configured. Concurrent callers wait on the lock and share one result.
    pub async fn online(&self) -> bool {
        if self.targets.is_empty() {
            return true;
        }
        let mut last = self.last.lock().await;
        if let Some((at, online)) = *last
            && at.elapsed() < CACHE
        {
            return online;
        }
        let online = reach_any(&self.targets).await;
        let was = last.map(|(_, online)| online);
        if !online && was != Some(false) {
            tracing::warn!(
                targets = ?self.targets,
                "Status host has no network; failed checks are not counted"
            );
        } else if online && was == Some(false) {
            tracing::info!("Status host network is back");
        }
        *last = Some((Instant::now(), online));
        online
    }
}

async fn reach_any(targets: &[String]) -> bool {
    let mut attempts = JoinSet::new();
    for target in targets {
        let target = target.clone();
        attempts.spawn(async move {
            matches!(
                tokio::time::timeout(TIMEOUT, TcpStream::connect(target.as_str())).await,
                Ok(Ok(_))
            )
        });
    }
    while let Some(result) = attempts.join_next().await {
        if result.unwrap_or(false) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn online_when_any_target_answers() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open = listener.local_addr().unwrap().to_string();
        let check = Connectivity::new(vec!["127.0.0.1:9".to_owned(), open]);
        assert!(check.online().await);
    }

    #[tokio::test]
    async fn offline_when_no_target_answers() {
        let check = Connectivity::new(vec!["127.0.0.1:9".to_owned()]);
        assert!(!check.online().await);
    }

    #[tokio::test]
    async fn no_targets_means_online() {
        assert!(Connectivity::new(Vec::new()).online().await);
    }
}
