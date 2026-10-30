use std::error::Error;
use std::fmt;
use std::time::Duration;

#[derive(Debug)]
struct ExampleError(&'static str);

impl fmt::Display for ExampleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for ExampleError {}

fn main() {
    let dsn = std::env::var("SENTRY_DSN").unwrap_or_else(|_| {
        eprintln!("Set SENTRY_DSN to the project DSN from the Owlee UI.");
        std::process::exit(1);
    });

    let _guard = sentry::init((
        dsn,
        sentry::ClientOptions {
            release: Some("rust-example@1.0.0".into()),
            environment: Some("production".into()),
            attach_stacktrace: true,
            ..Default::default()
        },
    ));

    sentry::configure_scope(|scope| {
        scope.set_user(Some(sentry::User {
            id: Some("42".into()),
            email: Some("ada@example.com".into()),
            username: Some("ada".into()),
            ..Default::default()
        }));
        scope.set_tag("example", "rust");
        scope.set_tag("service", "checkout");
    });

    sentry::add_breadcrumb(sentry::Breadcrumb {
        category: Some("example".into()),
        message: Some("capturing a manual error".into()),
        level: sentry::Level::Info,
        ..Default::default()
    });

    let error = ExampleError("manual error from the Rust example");
    sentry::capture_error(&error);

    let _ = std::panic::catch_unwind(|| {
        panic!("panic from the Rust example");
    });

    let flushed = sentry::Hub::current()
        .client()
        .map(|client| client.flush(Some(Duration::from_secs(10))))
        .unwrap_or(false);
    drop(_guard);
    if !flushed {
        eprintln!("sentry flush timed out");
        std::process::exit(1);
    }
}
