use console::style;
use omnidoc::{cli, OmniDocError, Result};
use std::any::Any;

const CLI_THREAD_STACK_SIZE: usize = 8 * 1024 * 1024;

fn main() {
    if let Some(code) = omnidoc::latex_recorder::run_wrapper_from_env() {
        std::process::exit(code);
    }
    if let Err(e) = run_cli() {
        eprintln!("{} Error: {}", style("✖").red().bold(), e);
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    let handle = std::thread::Builder::new()
        .name("omnidoc-cli".to_string())
        .stack_size(CLI_THREAD_STACK_SIZE)
        .spawn(cli::cli)
        .map_err(OmniDocError::Io)?;

    handle
        .join()
        .map_err(|payload| OmniDocError::Other(panic_payload_message(payload)))?
}

fn panic_payload_message(payload: Box<dyn Any + Send + 'static>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        format!("CLI thread panicked: {}", message)
    } else if let Some(message) = payload.downcast_ref::<String>() {
        format!("CLI thread panicked: {}", message)
    } else {
        "CLI thread panicked".to_string()
    }
}
