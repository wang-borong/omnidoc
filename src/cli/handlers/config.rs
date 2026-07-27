use crate::cli::commands::ConfigScope;
use crate::cli::handlers::common::{create_config_manager, print_json_error};
use crate::config::{CliOverrides, GlobalConfig, ProjectConfig};
use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use crate::utils::directories::config_local_dir;
use crate::utils::path;
use console::style;
use serde::Serialize;
use serde_json::Value;

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
    crate::utils::fs::create_dir_all(&config_local_dir)?;

    // If config exists and force is false, return error
    if crate::utils::fs::exists(&config_file) && !force {
        return Err(OmniDocError::Config(format!(
            "Configuration file already exists at {}. Use --force to overwrite.",
            config_file.display()
        )));
    }

    let mut config = GlobalConfig::default_schema()?;
    config.author.author = Some(AuthorSection {
        name: Some(authors),
    });

    // Set lib path if provided
    if let Some(lib_path) = lib {
        config.lib = LibConfig {
            lib: Some(LibSection {
                path: Some(lib_path),
            }),
        };
    }

    // Preserve first-run defaults for omitted values and override only the
    // fields the user explicitly supplied.
    let env = config.env.env.get_or_insert_with(EnvSection::default);
    if let Some(outdir) = outdir {
        env.outdir = Some(outdir);
    }
    if let Some(texmfhome) = texmfhome {
        env.texmfhome = Some(texmfhome);
    }
    if let Some(bibinputs) = bibinputs {
        env.bibinputs = Some(bibinputs);
    }
    if let Some(texinputs) = texinputs {
        env.texinputs = Some(texinputs);
    }

    // Write config
    let toml_content = toml::to_string_pretty(&config)
        .map_err(|e| OmniDocError::Config(format!("Failed to serialize config: {}", e)))?;

    crate::utils::fs::atomic_write(&config_file, toml_content.as_bytes())?;

    println!(
        "{} Configuration generated successfully at {}",
        style("✔").green().bold(),
        config_file.display()
    );
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ConfigSource {
    kind: String,
    path: String,
    exists: bool,
}

#[derive(Debug)]
struct ResolvedConfig {
    scope: String,
    sources: Vec<ConfigSource>,
    value: Value,
}

#[derive(Debug, Serialize)]
struct ConfigShowResponse<'a> {
    schema_version: u32,
    scope: &'a str,
    sources: &'a [ConfigSource],
    config: &'a Value,
}

#[derive(Debug, Serialize)]
struct ConfigGetResponse<'a> {
    schema_version: u32,
    scope: &'a str,
    key: &'a str,
    value: &'a Value,
    sources: &'a [ConfigSource],
}

pub fn handle_config_show(path: Option<String>, scope: ConfigScope, json: bool) -> Result<()> {
    let resolved = match resolve_config(path, scope) {
        Ok(resolved) => resolved,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        let response = ConfigShowResponse {
            schema_version: 1,
            scope: &resolved.scope,
            sources: &resolved.sources,
            config: &resolved.value,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&response)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    } else {
        print_config_sources(&resolved);
        println!(
            "{}",
            serde_json::to_string_pretty(&resolved.value)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    }
    Ok(())
}

pub fn handle_config_get(
    key: String,
    path: Option<String>,
    scope: ConfigScope,
    json: bool,
) -> Result<()> {
    let result = (|| {
        let resolved = resolve_config(path, scope)?;
        let value = resolve_key(&resolved.value, &key)?.clone();
        Ok((resolved, value))
    })();
    let (resolved, value) = match result {
        Ok(result) => result,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        let response = ConfigGetResponse {
            schema_version: 1,
            scope: &resolved.scope,
            key: &key,
            value: &value,
            sources: &resolved.sources,
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&response)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        );
    } else {
        print_config_value(&value)?;
    }
    Ok(())
}

fn resolve_config(requested_path: Option<String>, scope: ConfigScope) -> Result<ResolvedConfig> {
    match scope {
        ConfigScope::Global => {
            let global = GlobalConfig::load()?;
            let value = serde_json::to_value(global.get_config())
                .map_err(|error| OmniDocError::Other(error.to_string()))?;
            Ok(ResolvedConfig {
                scope: "global".to_string(),
                sources: vec![global_source(&global)],
                value,
            })
        }
        ConfigScope::Project => {
            let project_root = path::determine_project_root(requested_path)?;
            let project = ProjectConfig::load_from_path(Some(&project_root))?.ok_or_else(|| {
                OmniDocError::Config(format!(
                    "Project configuration not found at {}",
                    project_root.join(".omnidoc.toml").display()
                ))
            })?;
            let value = serde_json::to_value(project.get_config())
                .map_err(|error| OmniDocError::Other(error.to_string()))?;
            Ok(ResolvedConfig {
                scope: "project".to_string(),
                sources: vec![project_source(&project)],
                value,
            })
        }
        ConfigScope::Merged => {
            let project_context = path::determine_project_context(requested_path)?;
            let manager = create_config_manager(Some(&project_context), CliOverrides::new())?;
            let mut sources = vec![global_source(manager.global())];
            if let Some(project) = manager.project() {
                sources.push(project_source(project));
            }
            let value = serde_json::to_value(manager.get_merged())
                .map_err(|error| OmniDocError::Other(error.to_string()))?;
            Ok(ResolvedConfig {
                scope: "merged".to_string(),
                sources,
                value,
            })
        }
    }
}

fn global_source(global: &GlobalConfig) -> ConfigSource {
    ConfigSource {
        kind: "global".to_string(),
        path: global.path().to_string_lossy().to_string(),
        exists: global.exists(),
    }
}

fn project_source(project: &ProjectConfig) -> ConfigSource {
    ConfigSource {
        kind: "project".to_string(),
        path: project.path().to_string_lossy().to_string(),
        exists: true,
    }
}

fn resolve_key<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    if key.trim().is_empty() {
        return Err(OmniDocError::Config(
            "Configuration key cannot be empty".to_string(),
        ));
    }

    let mut current = value;
    for part in key.split('.') {
        current = current.get(part).ok_or_else(|| {
            let available = current
                .as_object()
                .map(|object| object.keys().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_else(|| "no child keys".to_string());
            OmniDocError::Config(format!(
                "Configuration key '{}' was not found at '{}'; available: {}",
                key, part, available
            ))
        })?;
    }
    Ok(current)
}

fn print_config_sources(resolved: &ResolvedConfig) {
    println!("scope: {}", resolved.scope);
    for source in &resolved.sources {
        println!(
            "source: {} {} ({})",
            source.kind,
            source.path,
            if source.exists { "file" } else { "defaults" }
        );
    }
}

fn print_config_value(value: &Value) -> Result<()> {
    match value {
        Value::String(value) => println!("{value}"),
        Value::Null => println!("null"),
        Value::Bool(value) => println!("{value}"),
        Value::Number(value) => println!("{value}"),
        Value::Array(_) | Value::Object(_) => println!(
            "{}",
            serde_json::to_string_pretty(value)
                .map_err(|error| OmniDocError::Other(error.to_string()))?
        ),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::resolve_key;

    #[test]
    fn resolves_dot_separated_configuration_keys() {
        let value = serde_json::json!({
            "project": { "target": "guide" },
            "outputs": ["pdf", "html"]
        });

        assert_eq!(
            resolve_key(&value, "project.target").expect("nested key"),
            "guide"
        );
        assert!(resolve_key(&value, "project.missing").is_err());
    }
}
