use std::process::ExitCode;

use owlee_platform::{TracingFormat, init_tracing};

fn main() -> ExitCode {
    let environment = std::env::var("OWLEE_ENV");
    let format = match environment.as_deref() {
        Ok("production") => TracingFormat::Json,
        _ => TracingFormat::Compact,
    };
    let _ = init_tracing(format);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match runtime.block_on(owlee_server::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("owlee: {error}");
            ExitCode::FAILURE
        }
    }
}
