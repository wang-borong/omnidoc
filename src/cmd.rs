use std::process::Command;
use std::io::{self, Write};

pub fn do_cmd(cmd: &str, args: &[&str]) -> io::Result<()>
{
    let output = Command::new(cmd)
        .args(args)
        .output()
        .expect(&format!("Failed to execute {}", cmd));
    io::stdout().write_all(&output.stdout)?;
    io::stderr().write_all(&output.stderr)?;

    Ok(())
}
