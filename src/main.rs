use console::style;
use omnidoc::cli;

fn main() {
    if let Err(e) = cli::cli() {
        eprintln!("{} Error: {}", style("✖").red().bold(), e);
        std::process::exit(1);
    }
}
