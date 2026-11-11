use owlee_platform::{Migration, MigrationRunner};

// `build.rs` makes `MIGRATIONS` from the files in `apps/server/migrations/`.
include!(concat!(env!("OUT_DIR"), "/migrations.rs"));

pub fn runner() -> MigrationRunner {
    MigrationRunner::new(
        env!("CARGO_PKG_VERSION"),
        MIGRATIONS
            .iter()
            .map(|&(version, name, sql)| Migration::new(version, name, sql))
            .collect(),
    )
}
