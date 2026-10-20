//! Status HTTP server, scheduler, and persistence.
//!
//! Mirrors Orbit's `apps/server`: a binary that migrates on boot, serves the
//! API, and embeds the built React app. There is no realtime layer.

mod checker;
mod cli;
mod engine;
mod error;
mod migrations;
mod notify;
mod present;
mod repo;
mod routes;
mod scheduler;
mod seed;
mod state;
mod static_assets;

pub use cli::run;
pub use routes::router;
pub use seed::{DEMO_EMAIL, DEMO_PASSWORD};
pub use state::{AppState, boot, testing_config};
