use crate::cli::commands::{ConfigScope, ConfigWriteScope};
use crate::cli::handlers::common::{create_config_manager, print_json_error};
use crate::config::{CliOverrides, ConfigSchema, GlobalConfig, ProjectConfig};
use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use crate::utils::directories::config_local_dir;
use crate::utils::path;
use console::style;
use serde::Serialize;
use serde_json::Value;
use similar::TextDiff;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, TableLike, Value as TomlValue};

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

#[derive(Debug, Clone, Copy)]
enum ConfigWriteOperation {
    Set,
    Unset,
}

impl ConfigWriteOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Set => "set",
            Self::Unset => "unset",
        }
    }
}

#[derive(Debug)]
struct ConfigWriteTarget {
    scope: &'static str,
    path: PathBuf,
    project_root: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct ConfigWriteReport {
    schema_version: u32,
    operation: &'static str,
    scope: &'static str,
    path: String,
    key: String,
    previous: Option<Value>,
    value: Option<Value>,
    changed: bool,
    created: bool,
    dry_run: bool,
    applied: bool,
    diff: Option<String>,
}

#[derive(Debug)]
struct PlannedConfigWrite {
    report: ConfigWriteReport,
    content: Option<String>,
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

#[allow(clippy::too_many_arguments)]
pub fn handle_config_set(
    key: String,
    value: String,
    path: Option<String>,
    scope: ConfigWriteScope,
    dry_run: bool,
    diff: bool,
    json: bool,
) -> Result<()> {
    handle_config_write(
        ConfigWriteOperation::Set,
        key,
        Some(value),
        path,
        scope,
        dry_run || diff,
        diff,
        json,
    )
}

pub fn handle_config_unset(
    key: String,
    path: Option<String>,
    scope: ConfigWriteScope,
    dry_run: bool,
    diff: bool,
    json: bool,
) -> Result<()> {
    handle_config_write(
        ConfigWriteOperation::Unset,
        key,
        None,
        path,
        scope,
        dry_run || diff,
        diff,
        json,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_config_write(
    operation: ConfigWriteOperation,
    key: String,
    raw_value: Option<String>,
    requested_path: Option<String>,
    scope: ConfigWriteScope,
    dry_run: bool,
    include_diff: bool,
    json: bool,
) -> Result<()> {
    let result = (|| {
        let key = key.trim().to_string();
        let segments = validate_write_key(&key, scope, operation)?;
        let value = raw_value
            .as_deref()
            .map(|raw| parse_config_value(&key, raw))
            .transpose()?;
        let target = resolve_write_target(requested_path, scope)?;
        let mut plan = plan_config_write(
            &target,
            operation,
            &key,
            &segments,
            value.as_ref(),
            dry_run,
            include_diff,
        )?;

        if !dry_run && plan.report.changed {
            let _project_lock = target
                .project_root
                .as_deref()
                .map(|root| {
                    crate::project_tools::acquire_project_write_lock(
                        root,
                        "update project configuration",
                    )
                })
                .transpose()?;

            if target.project_root.is_some() {
                plan = plan_config_write(
                    &target,
                    operation,
                    &key,
                    &segments,
                    value.as_ref(),
                    false,
                    include_diff,
                )?;
            }

            if plan.report.changed {
                let content = plan.content.as_deref().ok_or_else(|| {
                    OmniDocError::Config("Configuration change has no content to write".to_string())
                })?;
                if let Some(parent) = target.path.parent() {
                    crate::utils::fs::create_dir_all(parent)?;
                }
                crate::utils::fs::atomic_write(&target.path, content.as_bytes())?;
                validate_written_config(&target.path, &key)?;
                plan.report.applied = true;
            }
        }

        Ok(plan.report)
    })();

    let report = match result {
        Ok(report) => report,
        Err(error) => {
            if json {
                print_json_error(&error);
            }
            return Err(error);
        }
    };

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| {
                OmniDocError::Other(format!(
                    "Failed to serialize configuration change report: {error}"
                ))
            })?
        );
    } else {
        print_config_write_report(&report)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn plan_config_write(
    target: &ConfigWriteTarget,
    operation: ConfigWriteOperation,
    key: &str,
    segments: &[&str],
    value: Option<&TomlValue>,
    dry_run: bool,
    include_diff: bool,
) -> Result<PlannedConfigWrite> {
    let existed = target.path.is_file();
    let original = if existed {
        crate::utils::fs::read_to_string(&target.path)?
    } else {
        String::new()
    };
    let editable = if existed || matches!(operation, ConfigWriteOperation::Unset) {
        original.clone()
    } else if matches!(target.scope, "global") {
        let defaults = GlobalConfig::default_schema()?;
        toml::to_string_pretty(&defaults).map_err(|error| {
            OmniDocError::Config(format!(
                "Failed to prepare the default global configuration: {error}"
            ))
        })?
    } else {
        String::new()
    };

    let before = toml_document_json(&editable, &target.path)?;
    let previous = lookup_key(&before, key).cloned();
    let mut document = parse_edit_document(&editable, &target.path)?;

    match operation {
        ConfigWriteOperation::Set => {
            let value = value.ok_or_else(|| {
                OmniDocError::Config("A value is required for config set".to_string())
            })?;
            set_nested_item(
                document.as_table_mut(),
                segments,
                Item::Value(value.clone()),
                key,
            )?;
        }
        ConfigWriteOperation::Unset => {
            if previous.is_some() {
                remove_nested_item(document.as_table_mut(), segments, key)?;
            }
        }
    }

    let mut updated = document.to_string();
    let value = match operation {
        ConfigWriteOperation::Set => {
            let config = validate_config_schema(&updated, &target.path, key)?;
            ensure_schema_key(&config, key)?;
            let after = toml_document_json(&updated, &target.path)?;
            let value = lookup_key(&after, key).cloned().ok_or_else(|| {
                OmniDocError::Config(format!(
                    "Configuration key '{key}' was not present after editing"
                ))
            })?;
            validate_semantic_value(key, &value)?;
            Some(value)
        }
        ConfigWriteOperation::Unset if previous.is_some() => {
            validate_config_schema(&updated, &target.path, key)?;
            None
        }
        ConfigWriteOperation::Unset => None,
    };

    let changed = match operation {
        ConfigWriteOperation::Set => !existed || previous != value,
        ConfigWriteOperation::Unset => previous.is_some(),
    };
    if !changed {
        updated = original.clone();
    }

    let diff = if include_diff && changed {
        Some(config_unified_diff(
            &target.path,
            existed,
            &original,
            &updated,
        ))
    } else {
        None
    };
    let content = changed.then_some(updated);

    Ok(PlannedConfigWrite {
        report: ConfigWriteReport {
            schema_version: 1,
            operation: operation.as_str(),
            scope: target.scope,
            path: target.path.to_string_lossy().to_string(),
            key: key.to_string(),
            previous,
            value,
            changed,
            created: !existed && changed,
            dry_run,
            applied: false,
            diff,
        },
        content,
    })
}

fn resolve_write_target(
    requested_path: Option<String>,
    scope: ConfigWriteScope,
) -> Result<ConfigWriteTarget> {
    match scope {
        ConfigWriteScope::Global => {
            if requested_path.is_some() {
                return Err(OmniDocError::Config(
                    "PATH is only valid for project configuration; omit it with `--scope global`"
                        .to_string(),
                ));
            }
            let directory = config_local_dir().ok_or_else(|| {
                OmniDocError::Config("Local config directory not found".to_string())
            })?;
            Ok(ConfigWriteTarget {
                scope: "global",
                path: directory.join(config_consts::OMNIDOC_CONFIG_FILE),
                project_root: None,
            })
        }
        ConfigWriteScope::Project => {
            let project_root = path::determine_project_root(requested_path)?;
            Ok(ConfigWriteTarget {
                scope: "project",
                path: project_root.join(".omnidoc.toml"),
                project_root: Some(project_root),
            })
        }
    }
}

fn parse_config_value(key: &str, raw: &str) -> Result<TomlValue> {
    let candidate = format!("value = {raw}\n");
    if let Ok(document) = candidate.parse::<DocumentMut>() {
        if let Some(value) = document.get("value").and_then(Item::as_value) {
            let mut value = value.clone();
            value.decor_mut().clear();
            if !value.is_str() && !schema_accepts_value(key, &value) {
                let string = TomlValue::from(raw);
                if schema_accepts_value(key, &string) {
                    return Ok(string);
                }
            }
            return Ok(value);
        }
    }

    let mut value = TomlValue::from(raw);
    value.decor_mut().clear();
    Ok(value)
}

fn schema_accepts_value(key: &str, value: &TomlValue) -> bool {
    let segments = key.split('.').collect::<Vec<_>>();
    let mut document = DocumentMut::new();
    if set_nested_item(
        document.as_table_mut(),
        &segments,
        Item::Value(value.clone()),
        key,
    )
    .is_err()
    {
        return false;
    }
    let Ok(config) = toml::from_str::<ConfigSchema>(&document.to_string()) else {
        return false;
    };
    schema_contains_key(&config, key)
}

fn parse_edit_document(content: &str, path: &Path) -> Result<DocumentMut> {
    content.parse::<DocumentMut>().map_err(|error| {
        OmniDocError::Config(format!(
            "Failed to parse configuration at {}: {error}",
            path.display()
        ))
    })
}

fn toml_document_json(content: &str, path: &Path) -> Result<Value> {
    let value: toml::Value = toml::from_str(content).map_err(|error| {
        OmniDocError::Config(format!(
            "Failed to parse configuration at {}: {error}",
            path.display()
        ))
    })?;
    serde_json::to_value(value).map_err(|error| OmniDocError::Other(error.to_string()))
}

fn validate_config_schema(content: &str, path: &Path, key: &str) -> Result<ConfigSchema> {
    let config = toml::from_str::<ConfigSchema>(content).map_err(|error| {
        OmniDocError::Config(format!(
            "Value for '{key}' is not valid in {}: {error}",
            path.display()
        ))
    })?;
    if let Some(theme) = config
        .theme
        .as_ref()
        .and_then(|config| config.theme.as_ref())
    {
        if theme.name.is_none() && (theme.version.is_some() || theme.compatibility.is_some()) {
            return Err(OmniDocError::Config(
                "theme.version and theme.compatibility require theme.name; set theme.name first or unset the whole theme section"
                    .to_string(),
            ));
        }
    }
    Ok(config)
}

fn validate_written_config(path: &Path, key: &str) -> Result<()> {
    let content = crate::utils::fs::read_to_string(path)?;
    validate_config_schema(&content, path, key).map(|_| ())
}

fn ensure_schema_key(config: &ConfigSchema, key: &str) -> Result<()> {
    if schema_contains_key(config, key) {
        Ok(())
    } else {
        Err(OmniDocError::Config(format!(
            "Unknown configuration key '{key}'; use a key from `omnidoc config set --help` or the .omnidoc.toml schema"
        )))
    }
}

fn schema_contains_key(config: &ConfigSchema, key: &str) -> bool {
    serde_json::to_value(config)
        .ok()
        .and_then(|value| lookup_key(&value, key).cloned())
        .is_some()
}

fn set_nested_item(
    table: &mut dyn TableLike,
    segments: &[&str],
    mut value: Item,
    key: &str,
) -> Result<()> {
    let (segment, remaining) = segments
        .split_first()
        .ok_or_else(|| OmniDocError::Config("Configuration key cannot be empty".to_string()))?;
    if remaining.is_empty() {
        if let Some(existing) = table.get_mut(segment) {
            if existing.is_table() || (existing.is_array_of_tables() && key != "download") {
                return Err(OmniDocError::Config(format!(
                    "Configuration key '{key}' identifies a section; set one of its child keys instead"
                )));
            }
            if let (Some(existing_value), Some(new_value)) =
                (existing.as_value(), value.as_value_mut())
            {
                *new_value.decor_mut() = existing_value.decor().clone();
            }
            *existing = value;
        } else {
            table.insert(segment, value);
        }
        return Ok(());
    }

    if !table.contains_key(segment) {
        table.insert(segment, Item::Table(Table::new()));
    }
    let child = table.get_mut(segment).ok_or_else(|| {
        OmniDocError::Config(format!(
            "Could not create configuration section '{}'",
            segments[..segments.len() - remaining.len()].join(".")
        ))
    })?;
    let child_type = child.type_name();
    let child_table = child.as_table_like_mut().ok_or_else(|| {
        OmniDocError::Config(format!(
            "Cannot set '{key}' because '{}' is a {child_type} instead of a table",
            segment
        ))
    })?;
    set_nested_item(child_table, remaining, value, key)
}

fn remove_nested_item(table: &mut dyn TableLike, segments: &[&str], key: &str) -> Result<()> {
    let (segment, remaining) = segments
        .split_first()
        .ok_or_else(|| OmniDocError::Config("Configuration key cannot be empty".to_string()))?;
    if remaining.is_empty() {
        table.remove(segment);
        return Ok(());
    }

    let Some(child) = table.get_mut(segment) else {
        return Ok(());
    };
    let child_type = child.type_name();
    let child_table = child.as_table_like_mut().ok_or_else(|| {
        OmniDocError::Config(format!(
            "Cannot unset '{key}' because '{}' is a {child_type} instead of a table",
            segment
        ))
    })?;
    remove_nested_item(child_table, remaining, key)
}

fn validate_write_key(
    key: &str,
    scope: ConfigWriteScope,
    operation: ConfigWriteOperation,
) -> Result<Vec<&str>> {
    if key.is_empty() {
        return Err(OmniDocError::Config(
            "Configuration key cannot be empty".to_string(),
        ));
    }
    let segments = key.split('.').collect::<Vec<_>>();
    if segments.iter().any(|segment| {
        segment.is_empty()
            || !segment
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
    }) {
        return Err(OmniDocError::Config(format!(
            "Invalid configuration key '{key}'; use dot-separated letters, numbers, and underscores"
        )));
    }

    if !is_known_write_root(segments[0]) {
        return Err(OmniDocError::Config(format!(
            "Unknown configuration root '{}'; use a schema key such as project.target, build.outputs, theme.name, or tools.pandoc",
            segments[0]
        )));
    }

    if !key_allowed_in_scope(&segments, scope) {
        let suggested = match scope {
            ConfigWriteScope::Global => "project",
            ConfigWriteScope::Project => "global",
        };
        return Err(OmniDocError::Config(format!(
            "Configuration key '{key}' is not used in the selected {} scope; use `--scope {suggested}`",
            write_scope_name(scope)
        )));
    }

    if matches!(segments.as_slice(), ["pandoc", "format_options", format]
        if !crate::project_tools::supported_outputs().contains(format))
    {
        return Err(OmniDocError::Config(format!(
            "Unsupported pandoc.format_options key '{}'; choose pdf, html, epub, docx, pptx, or latex",
            segments[2]
        )));
    }
    if segments.starts_with(&["pandoc", "format_options"]) && segments.len() > 3 {
        return Err(OmniDocError::Config(format!(
            "Configuration key '{key}' is too deeply nested; use a key such as pandoc.format_options.html"
        )));
    }

    if matches!(operation, ConfigWriteOperation::Set) {
        if let Some(example) = section_child_example(&segments) {
            return Err(OmniDocError::Config(format!(
                "'{key}' is a configuration section; set a child key such as '{example}'"
            )));
        }
    }

    Ok(segments)
}

fn is_known_write_root(root: &str) -> bool {
    matches!(
        root,
        "author"
            | "lib"
            | "env"
            | "project"
            | "build"
            | "figure"
            | "pandoc"
            | "theme"
            | "tools"
            | "tectonic"
            | "paths"
            | "template_dir"
            | "download"
    )
}

fn section_child_example(segments: &[&str]) -> Option<&'static str> {
    match segments {
        ["author"] => Some("author.name"),
        ["lib"] => Some("lib.path"),
        ["env"] => Some("env.outdir"),
        ["project"] => Some("project.target"),
        ["build"] => Some("build.outputs"),
        ["figure"] => Some("figure.paths"),
        ["pandoc"] => Some("pandoc.toc"),
        ["pandoc", "format_options"] => Some("pandoc.format_options.html"),
        ["theme"] => Some("theme.name"),
        ["tools"] => Some("tools.pandoc"),
        ["tectonic"] => Some("tectonic.only_cached"),
        ["paths"] => Some("paths.build_dir"),
        _ => None,
    }
}

fn key_allowed_in_scope(segments: &[&str], scope: ConfigWriteScope) -> bool {
    let root = segments[0];
    match scope {
        ConfigWriteScope::Global => matches!(
            root,
            "author" | "lib" | "env" | "theme" | "tools" | "tectonic" | "paths" | "template_dir"
        ),
        ConfigWriteScope::Project => matches!(
            root,
            "author"
                | "project"
                | "build"
                | "figure"
                | "pandoc"
                | "theme"
                | "tools"
                | "tectonic"
                | "paths"
                | "download"
        ),
    }
}

fn write_scope_name(scope: ConfigWriteScope) -> &'static str {
    match scope {
        ConfigWriteScope::Global => "global",
        ConfigWriteScope::Project => "project",
    }
}

fn validate_semantic_value(key: &str, value: &Value) -> Result<()> {
    match key {
        "project.from" => {
            let value = value.as_str().unwrap_or_default();
            if !matches!(
                value.to_ascii_lowercase().as_str(),
                "markdown" | "md" | "latex" | "tex"
            ) {
                return Err(OmniDocError::Config(format!(
                    "Unsupported project.from '{value}'; choose markdown, md, latex, or tex"
                )));
            }
        }
        "project.to" => validate_output_name("project.to", value)?,
        "build.outputs" => {
            let outputs = value.as_array().ok_or_else(|| {
                OmniDocError::Config(
                    "build.outputs must be an array such as '[\"pdf\", \"html\"]'".to_string(),
                )
            })?;
            for output in outputs {
                validate_output_name("build.outputs", output)?;
            }
        }
        "build.latex_backend" => {
            let backend = value.as_str().unwrap_or_default();
            if !matches!(backend.to_ascii_lowercase().as_str(), "latexmk" | "engine") {
                return Err(OmniDocError::Config(format!(
                    "Unsupported build.latex_backend '{backend}'; choose latexmk or engine"
                )));
            }
        }
        "build.max_latex_passes" if value.as_u64() == Some(0) => {
            return Err(OmniDocError::Config(
                "build.max_latex_passes must be greater than 0".to_string(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_output_name(key: &str, value: &Value) -> Result<()> {
    let output = value.as_str().ok_or_else(|| {
        OmniDocError::Config(format!("{key} must contain output names as strings"))
    })?;
    if crate::project_tools::supported_outputs().contains(&output.to_ascii_lowercase().as_str()) {
        Ok(())
    } else {
        Err(OmniDocError::Config(format!(
            "Unsupported {key} value '{output}'; choose pdf, html, epub, docx, pptx, or latex"
        )))
    }
}

fn config_unified_diff(path: &Path, existed: bool, before: &str, after: &str) -> String {
    let path = path.to_string_lossy();
    let old_label = if existed {
        format!("a/{path}")
    } else {
        "/dev/null".to_string()
    };
    let new_label = format!("b/{path}");
    TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&old_label, &new_label)
        .to_string()
}

fn print_config_write_report(report: &ConfigWriteReport) -> Result<()> {
    if !report.changed {
        match report.operation {
            "set" => println!(
                "Configuration key '{}' already has the requested value in {}.",
                report.key, report.path
            ),
            "unset" => println!(
                "Configuration key '{}' is not set in {}.",
                report.key, report.path
            ),
            _ => {}
        }
        println!("No files were changed.");
        return Ok(());
    }

    let action = match (report.dry_run, report.operation) {
        (true, "set") => "Would set",
        (true, "unset") => "Would unset",
        (false, "set") => "Set",
        (false, "unset") => "Unset",
        _ => "Updated",
    };
    println!(
        "{action} {} configuration key '{}' in {}.",
        report.scope, report.key, report.path
    );
    if let Some(previous) = &report.previous {
        print!("  Previous: ");
        print_config_value(previous)?;
    }
    if let Some(value) = &report.value {
        print!("  Value:    ");
        print_config_value(value)?;
    }
    if let Some(diff) = &report.diff {
        print!("{diff}");
        if !diff.ends_with('\n') {
            println!();
        }
    }
    if report.dry_run {
        println!("No files were changed.");
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

fn lookup_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current)
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
    use super::{
        parse_config_value, remove_nested_item, resolve_key, set_nested_item, validate_write_key,
        ConfigWriteOperation,
    };
    use crate::cli::commands::ConfigWriteScope;
    use toml_edit::{DocumentMut, Item};

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

    #[test]
    fn parses_toml_values_and_falls_back_to_strings() {
        assert!(parse_config_value("build.verbose", "true")
            .expect("boolean")
            .is_bool());
        assert_eq!(
            parse_config_value("build.max_latex_passes", "42")
                .expect("integer")
                .as_integer(),
            Some(42)
        );
        assert_eq!(
            parse_config_value("build.outputs", "[\"pdf\", \"html\"]")
                .expect("array")
                .as_array()
                .map(|array| array.len()),
            Some(2)
        );
        assert_eq!(
            parse_config_value("author.name", "Docs Team")
                .expect("string")
                .as_str(),
            Some("Docs Team")
        );
        assert_eq!(
            parse_config_value("theme.version", "1")
                .expect("numeric-looking string")
                .as_str(),
            Some("1")
        );
        assert_eq!(
            parse_config_value("project.target", "2026-07-27")
                .expect("date-looking string")
                .as_str(),
            Some("2026-07-27")
        );
    }

    #[test]
    fn edits_nested_values_without_losing_comments_or_layout() {
        let original = concat!(
            "# project settings\n",
            "[project]\n",
            "target = 'old' # keep this explanation\n",
            "\n",
            "[build]\n",
            "outputs = [\"html\"]\n",
        );
        let mut document = original.parse::<DocumentMut>().expect("document");
        let value = parse_config_value("project.target", "guide").expect("value");

        set_nested_item(
            document.as_table_mut(),
            &["project", "target"],
            Item::Value(value),
            "project.target",
        )
        .expect("set target");
        let edited = document.to_string();

        assert!(edited.contains("# project settings"));
        assert!(edited.contains("target = \"guide\" # keep this explanation"));
        assert!(edited.contains("\n\n[build]\n"));

        remove_nested_item(
            document.as_table_mut(),
            &["project", "target"],
            "project.target",
        )
        .expect("unset target");
        let edited = document.to_string();
        assert!(edited.contains("# project settings"));
        assert!(edited.contains("[project]\n\n[build]"));
    }

    #[test]
    fn write_keys_reject_ignored_scopes_and_allow_section_resets() {
        let error = validate_write_key(
            "build.outputs",
            ConfigWriteScope::Global,
            ConfigWriteOperation::Set,
        )
        .expect_err("project-only key");
        assert!(error.to_string().contains("--scope project"));

        validate_write_key(
            "theme",
            ConfigWriteScope::Project,
            ConfigWriteOperation::Unset,
        )
        .expect("section reset");
        assert!(validate_write_key(
            "theme",
            ConfigWriteScope::Project,
            ConfigWriteOperation::Set,
        )
        .is_err());
    }
}
