use crate::config::ConfigParser;
use crate::error::{OmniDocError, Result};
use console::style;

/// Handle the 'config' command
pub fn handle_config(
    authors: String,
    lib: Option<String>,
    outdir: Option<String>,
    texmfhome: Option<String>,
    bibinputs: Option<String>,
    texinputs: Option<String>,
    force: bool,
) -> Result<()> {
    ConfigParser::gen(authors, lib, outdir, texmfhome, bibinputs, texinputs, force)
        .map_err(|e| OmniDocError::Config(format!("Generate configuration failed: {}", e)))?;
    println!(
        "{} Generate configuration success",
        style("✔").green().bold()
    );
    Ok(())
}
