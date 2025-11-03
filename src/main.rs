use omnidoc::cli;

fn main() {
    if let Err(e) = cli::cli() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
