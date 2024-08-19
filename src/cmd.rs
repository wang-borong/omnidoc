use std::io::{self, Write};
use std::process::Command;

pub fn do_cmd(cmd: &str, args: &[&str], nw: bool) -> io::Result<()> {
    if nw {
        Command::new(cmd).args(args).spawn()?;
    } else {
        let output = Command::new(cmd)
            .args(args)
            .output()
            .expect(&format!("Failed to execute '{}'", cmd));
        io::stdout().write_all(&output.stdout)?;
        io::stderr().write_all(&output.stderr)?;
    }

    Ok(())
}
