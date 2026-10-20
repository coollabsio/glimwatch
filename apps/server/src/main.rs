use std::process::ExitCode;

use coolstatus_platform::{TracingFormat, init_tracing};

fn main() -> ExitCode {
    let format = match std::env::var("COOLSTATUS_ENV").as_deref() {
        Ok("production") => TracingFormat::Json,
        _ => TracingFormat::Compact,
    };
    let _ = init_tracing(format);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match runtime.block_on(coolstatus_server::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("coolstatus: {error}");
            ExitCode::FAILURE
        }
    }
}
