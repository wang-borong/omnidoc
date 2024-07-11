mod cli;

fn main() {
    let r = cli::cli();
    match r {
        Err(e) => {eprintln!("{}", e)},
        Ok(()) => {},
    }
}
