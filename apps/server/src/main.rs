use std::process::ExitCode;

use status_platform::{TracingFormat, init_tracing};

fn main() -> ExitCode {
    let format = match std::env::var("STATUS_ENV").as_deref() {
        Ok("production") => TracingFormat::Json,
        _ => TracingFormat::Compact,
    };
    let _ = init_tracing(format);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match runtime.block_on(status_server::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("status: {error}");
            ExitCode::FAILURE
        }
    }
}
