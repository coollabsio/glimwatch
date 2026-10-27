use std::process::ExitCode;

use statixo_platform::{TracingFormat, init_tracing};

fn main() -> ExitCode {
    let environment = std::env::var("STATIXO_ENV")
        .or_else(|_| std::env::var("HARUVO_ENV"))
        .or_else(|_| std::env::var("STATUS_ENV"));
    let format = match environment.as_deref() {
        Ok("production") => TracingFormat::Json,
        _ => TracingFormat::Compact,
    };
    let _ = init_tracing(format);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match runtime.block_on(statixo_server::run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("statixo: {error}");
            ExitCode::FAILURE
        }
    }
}
