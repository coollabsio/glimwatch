use status_platform::{Migration, MigrationRunner};

pub fn runner() -> MigrationRunner {
    MigrationRunner::new(
        env!("CARGO_PKG_VERSION"),
        vec![
            Migration::new(
                1,
                "identity",
                include_str!("../migrations/0001_identity.sql"),
            ),
            Migration::new(
                2,
                "monitors",
                include_str!("../migrations/0002_monitors.sql"),
            ),
            Migration::new(3, "checks", include_str!("../migrations/0003_checks.sql")),
            Migration::new(4, "pages", include_str!("../migrations/0004_pages.sql")),
            Migration::new(
                5,
                "incidents",
                include_str!("../migrations/0005_incidents.sql"),
            ),
            Migration::new(
                6,
                "maintenance",
                include_str!("../migrations/0006_maintenance.sql"),
            ),
            Migration::new(
                7,
                "notifications",
                include_str!("../migrations/0007_notifications.sql"),
            ),
            Migration::new(8, "audit", include_str!("../migrations/0008_audit.sql")),
        ],
    )
}
