use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use console::style;
use dirs::config_local_dir;

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
    use crate::config::schema::*;

    let config_local_dir = config_local_dir()
        .ok_or_else(|| OmniDocError::Config("Local config directory not found".to_string()))?;
    let config_file = config_local_dir.join(config_consts::OMNIDOC_CONFIG_FILE);

    // If config exists and force is false, return error
    if crate::utils::fs::exists(&config_file) && !force {
        return Err(OmniDocError::Config(format!(
            "Configuration file already exists at {}. Use --force to overwrite.",
            config_file.display()
        )));
    }

    let mut config = ConfigSchema::default();

    // Set author
    config.author = AuthorConfig {
        author: Some(AuthorSection {
            name: Some(authors),
        }),
    };

    // Set lib path if provided
    if let Some(lib_path) = lib {
        config.lib = LibConfig {
            lib: Some(LibSection {
                path: Some(lib_path),
                url: Some("https://github.com/wang-borong/omnidoc-libs".to_string()),
            }),
        };
    }

    // Set environment variables
    config.env = EnvConfig {
        env: Some(EnvSection {
            outdir,
            texmfhome,
            bibinputs,
            texinputs,
        }),
    };

    // Write config
    let toml_content = toml::to_string_pretty(&config)
        .map_err(|e| OmniDocError::Config(format!("Failed to serialize config: {}", e)))?;

    crate::utils::fs::write(&config_file, toml_content.as_bytes())?;

    println!(
        "{} Configuration generated successfully at {}",
        style("✔").green().bold(),
        config_file.display()
    );
    Ok(())
}
