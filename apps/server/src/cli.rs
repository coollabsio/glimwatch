use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use coolstatus_platform::Config;

use crate::scheduler;
use crate::seed;
use crate::state::{self, BootError};

#[derive(Parser)]
#[command(
    name = "coolstatus",
    version,
    about = "Status pages, incidents, and uptime monitoring"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP server and the check scheduler.
    Serve {
        /// Listen address, for example 127.0.0.1:43123.
        #[arg(long)]
        listen: Option<SocketAddr>,
        /// SQLite database file. Ignored when a Postgres URL is configured.
        #[arg(long)]
        database: Option<PathBuf>,
    },
    /// Apply migrations and exit.
    Migrate {
        #[arg(long)]
        database: Option<PathBuf>,
    },
    /// Create the demo account, monitors, status page, and incident history.
    Seed {
        #[arg(long)]
        database: Option<PathBuf>,
    },
    /// Exit successfully when /api/health answers on the configured port.
    Healthcheck,
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve {
        listen: None,
        database: None,
    }) {
        Command::Serve { listen, database } => {
            let config = load(listen, database)?;
            let addr = config.listen_addr();
            let state = state::boot(config).await?;
            scheduler::spawn(state.clone());
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let bound = listener.local_addr()?;
            tracing::info!("CoolStatus listening on http://{bound}");
            axum::serve(listener, crate::routes::router(state))
                .with_graceful_shutdown(async {
                    let _ = tokio::signal::ctrl_c().await;
                    tracing::info!("shutting down");
                })
                .await?;
        }
        Command::Migrate { database } => {
            let config = load(None, database)?;
            state::boot(config).await?;
            println!("migrations applied");
        }
        Command::Seed { database } => {
            let config = load(None, database)?;
            let state = state::boot(config).await?;
            if seed::seed(&state).await? {
                println!("seeded {DEMO}", DEMO = seed::DEMO_EMAIL);
                println!("password: {}", seed::DEMO_PASSWORD);
            } else {
                println!("database already has an account; left it unchanged");
            }
        }
        Command::Healthcheck => {
            let config = Config::from_env()?;
            let url = format!("http://127.0.0.1:{}/api/health", config.port);
            let response = reqwest::Client::builder()
                .timeout(Duration::from_secs(3))
                .build()?
                .get(url)
                .send()
                .await?;
            if !response.status().is_success() {
                return Err(format!("health returned {}", response.status()).into());
            }
        }
    }
    Ok(())
}

fn load(listen: Option<SocketAddr>, database: Option<PathBuf>) -> Result<Config, BootError> {
    let mut config = Config::from_env().map_err(|error| BootError::Config(error.to_string()))?;
    if let Some(listen) = listen {
        config.apply_listen(listen);
    }
    if let Some(database) = database {
        config.database_path = database;
    }
    Ok(config)
}
