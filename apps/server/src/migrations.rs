use owlee_platform::{Migration, MigrationRunner};

pub fn runner() -> MigrationRunner {
    MigrationRunner::new(
        env!("CARGO_PKG_VERSION"),
        vec![Migration::new(
            1,
            "initial",
            include_str!("../migrations/0001_initial.sql"),
        )],
    )
}
