//! Owlee HTTP server, scheduler, and persistence.
//!
//! Mirrors Orbit's `apps/server`: a binary that migrates on boot, serves the
//! API, and embeds the built React app. There is no realtime layer.

mod agent;
mod checker;
mod cli;
mod connectivity;
mod dashboard;
mod dns;
mod engine;
mod error;
mod errors;
mod insights;
mod instance_settings;
mod limit;
mod migrations;
mod notify;
mod ownership;
mod peer;
mod present;
mod quota;
mod repo;
mod retention;
mod routes;
mod scheduler;
mod seed;
mod setup;
mod state;
mod static_assets;
mod verify;

pub use cli::run;
pub use dns::MapResolver;
pub use errors::seed_demo as seed_errors;
pub use instance_settings::permit_signup;
pub use quota::today as quota_today;
pub use retention::prune_before;
pub use routes::router;
pub use seed::{DEMO_EMAIL, DEMO_PASSWORD};
pub use setup::{SetupError, initialize_setup_token, is_complete, rotate_setup_token, setup_url};
pub use state::{AppState, boot, testing_config};
pub use verify::recheck_verified;
