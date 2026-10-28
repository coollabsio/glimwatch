use glimwatch_platform::{Migration, MigrationRunner};

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
            Migration::new(9, "agents", include_str!("../migrations/0009_agents.sql")),
            Migration::new(
                10,
                "insights",
                include_str!("../migrations/0010_insights.sql"),
            ),
            Migration::new(11, "errors", include_str!("../migrations/0011_errors.sql")),
            Migration::new(12, "setup", include_str!("../migrations/0012_setup.sql")),
            Migration::new(
                13,
                "instance_settings",
                include_str!("../migrations/0013_instance_settings.sql"),
            ),
        ],
    )
}
