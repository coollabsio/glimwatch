use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use statixo_platform::{Config, TimestampMillis};

use crate::agent;
use crate::connectivity::Connectivity;
use crate::ownership;
use crate::scheduler;
use crate::seed;
use crate::setup;
use crate::state::{self, BootError};

#[derive(Parser)]
#[command(
    name = "statixo",
    version,
    about = "Statixo status pages, incidents, and uptime monitoring"
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
    /// One-time setup URL for the first owner account.
    SetupToken {
        /// SQLite database file. Ignored when a Postgres URL is configured.
        #[arg(long)]
        database: Option<PathBuf>,
        #[command(subcommand)]
        command: SetupTokenCommand,
    },
    /// Run the checks in a config file from this network and report them to a Statixo server.
    Agent {
        /// Statixo server URL, for example https://status.example.com.
        /// `HARUVO_AGENT_SERVER` and `STATUS_AGENT_SERVER` are used when this is unset.
        #[arg(long, env = "STATIXO_AGENT_SERVER")]
        server: String,
        /// Token shown once when the agent was created.
        /// `HARUVO_AGENT_TOKEN` and `STATUS_AGENT_TOKEN` are used when this is unset.
        #[arg(long, env = "STATIXO_AGENT_TOKEN", hide_env_values = true)]
        token: String,
        /// TOML file with [[check]] entries.
        /// `HARUVO_AGENT_CONFIG` and `STATUS_AGENT_CONFIG` are used when this is unset.
        #[arg(long, env = "STATIXO_AGENT_CONFIG")]
        config: PathBuf,
    },
}

#[derive(Subcommand)]
enum SetupTokenCommand {
    /// Print a setup URL when setup is unfinished and no unused token is stored.
    Init {
        /// Public origin to put in the URL. Defaults to STATIXO_PUBLIC_ORIGIN.
        /// `HARUVO_PUBLIC_ORIGIN` and `STATUS_PUBLIC_ORIGIN` still work.
        #[arg(long)]
        origin: Option<String>,
    },
    /// Replace the unused setup token. Refuses after setup is complete.
    /// On SQLite, stop Statixo first. On Postgres, this can run while Statixo is up.
    Rotate {
        /// Public origin to put in the URL. Defaults to STATIXO_PUBLIC_ORIGIN.
        /// `HARUVO_PUBLIC_ORIGIN` and `STATUS_PUBLIC_ORIGIN` still work.
        #[arg(long)]
        origin: Option<String>,
    },
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    promote_legacy_agent_env();
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve {
        listen: None,
        database: None,
    }) {
        Command::Serve { listen, database } => {
            let config = load(listen, database)?;
            let _sqlite_owner = sqlite_owner(&config)?;
            let addr = config.listen_addr();
            let state = state::boot(config).await?;
            if let Some(issued) =
                setup::initialize_setup_token(&state.db, TimestampMillis::now()).await?
            {
                let url = setup::setup_url(&state.config.public_origin, &issued.token);
                eprintln!("Initial setup URL: {url}");
            }
            scheduler::spawn(state.clone());
            let listener = tokio::net::TcpListener::bind(addr).await?;
            let bound = listener.local_addr()?;
            tracing::info!("Statixo listening on http://{bound}");
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
        Command::Agent {
            server,
            token,
            config,
        } => {
            let targets = Config::from_env()?.connectivity_targets;
            agent::run(&server, token, &config, Connectivity::new(targets)).await?;
        }
        Command::SetupToken { database, command } => {
            let config = load(None, database)?;
            let _sqlite_owner = sqlite_owner(&config)?;
            let origin = match &command {
                SetupTokenCommand::Init { origin } | SetupTokenCommand::Rotate { origin } => {
                    origin.clone()
                }
            };
            let state = state::boot(config).await?;
            let origin = origin.unwrap_or_else(|| state.config.public_origin.clone());
            let now = TimestampMillis::now();
            match command {
                SetupTokenCommand::Init { .. } => {
                    match setup::initialize_setup_token(&state.db, now).await? {
                        Some(issued) => println!("{}", setup::setup_url(&origin, &issued.token)),
                        None => {
                            println!("setup token already exists or setup is complete");
                        }
                    }
                }
                SetupTokenCommand::Rotate { .. } => {
                    let issued = setup::rotate_setup_token(&state.db, now).await?;
                    println!("{}", setup::setup_url(&origin, &issued.token));
                }
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

fn promote_legacy_agent_env() {
    for suffix in ["AGENT_SERVER", "AGENT_TOKEN", "AGENT_CONFIG"] {
        let primary = format!("STATIXO_{suffix}");
        if std::env::var_os(&primary).is_some() {
            continue;
        }
        for prefix in ["HARUVO_", "STATUS_"] {
            let legacy = format!("{prefix}{suffix}");
            if let Some(value) = std::env::var_os(&legacy) {
                // SAFETY: process startup, before other threads read these variables.
                unsafe { std::env::set_var(&primary, value) };
                break;
            }
        }
    }
}

/// SQLite is owned by one Statixo process. Postgres stays unlocked so `setup-token`
/// can run while the server is up, matching Orbit.
fn sqlite_owner(config: &Config) -> Result<Option<ownership::Guard>, BootError> {
    if config.database_url.is_some() {
        return Ok(None);
    }
    ownership::lock_sqlite(&config.database_path)
        .map(Some)
        .map_err(BootError::Config)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_setup_token_rotate() {
        let cli = Cli::try_parse_from([
            "statixo",
            "setup-token",
            "rotate",
            "--origin",
            "https://status.example.com",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::SetupToken {
                command: SetupTokenCommand::Rotate { .. },
                ..
            })
        ));
    }
}
