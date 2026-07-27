use crate::config::global::GlobalConfig;
use crate::doc::templates::{
    list_project_templates, resolve_project_template, ProjectTemplateInfo, TemplateSource,
    DEFAULT_TEMPLATE_KEY,
};
use crate::doctype::DocumentFormat;
use crate::error::{OmniDocError, Result};
use console::style;
use inquire::Select;
use std::io::{self, IsTerminal};
use std::path::Path;

/// Print all supported document types
pub fn print_doctypes() {
    let _ = print_templates(None, false);
}

pub fn print_templates(format: Option<DocumentFormat>, json: bool) -> Result<()> {
    let templates = list_project_templates()
        .into_iter()
        .filter(|template| format.is_none_or(|format| template.format == format))
        .collect::<Vec<_>>();

    if json {
        let content = serde_json::to_string_pretty(&templates)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        println!("{}", content);
        return Ok(());
    }

    println!(
        "{} ({} available)",
        style("Project templates").bold().underlined(),
        templates.len()
    );
    for current_format in [DocumentFormat::Markdown, DocumentFormat::Latex] {
        let matching = templates
            .iter()
            .filter(|template| template.format == current_format)
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        println!("\n{}", style(current_format.as_str()).bold());
        for template in matching {
            let source = match template.source {
                TemplateSource::BuiltIn => "built-in",
                TemplateSource::External => "external",
            };
            let summary = template_summary(template);
            if summary.is_empty() {
                println!("  {:<18} [{}]", template.key, source);
            } else {
                println!("  {:<18} {} [{}]", template.key, summary, source);
            }
        }
    }
    Ok(())
}

/// Prompt for a project template without mutating the target path.
pub fn get_doctype_from_readline(format: Option<DocumentFormat>) -> Result<String> {
    if !io::stdin().is_terminal() {
        return Err(OmniDocError::Other(
            "A template must be selected in non-interactive mode. Use `--type <KEY>` or `--defaults`; run `omnidoc template list` to see available keys."
                .to_string(),
        ));
    }

    let templates = list_project_templates()
        .into_iter()
        .filter(|template| format.is_none_or(|format| template.format == format))
        .collect::<Vec<_>>();
    if templates.is_empty() {
        return Err(OmniDocError::Other(format!(
            "No {} templates are available",
            format.map_or("project", DocumentFormat::as_str)
        )));
    }

    let mut items = templates
        .iter()
        .map(|template| {
            let source = match template.source {
                TemplateSource::BuiltIn => "built-in",
                TemplateSource::External => "external",
            };
            let summary = template_summary(template);
            if summary.is_empty() {
                format!(
                    "{:<8} {} [{}]",
                    template.format.as_str(),
                    template.key,
                    source
                )
            } else {
                format!(
                    "{:<8} {} — {} [{}]",
                    template.format.as_str(),
                    template.key,
                    summary,
                    source
                )
            }
        })
        .collect::<Vec<_>>();
    items.push("[Cancel]".to_string());

    let selection = Select::new("Select project template:", items.clone())
        .with_page_size(10)
        .with_help_message(
            "Type to filter, use arrow keys to navigate, Enter to confirm, Esc/Ctrl+C to cancel",
        )
        .prompt();

    let selection = match selection {
        Ok(sel) => sel,
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            return Err(OmniDocError::Other("Operation canceled".to_string()));
        }
        Err(e) => {
            return Err(OmniDocError::Other(format!("Failed to prompt user: {}", e)));
        }
    };

    if selection.starts_with("[Cancel]") {
        return Err(OmniDocError::Other("Operation canceled".to_string()));
    }

    let selected_index = items
        .iter()
        .position(|item| item == &selection)
        .ok_or_else(|| OmniDocError::Other("Selected template was not found".to_string()))?;
    Ok(templates[selected_index].key.clone())
}

fn template_summary(template: &ProjectTemplateInfo) -> String {
    match (
        template.name != template.key,
        template.description.is_empty(),
    ) {
        (true, false) => format!("{} — {}", template.name, template.description),
        (true, true) => template.name.clone(),
        (false, false) => template.description.clone(),
        (false, true) => String::new(),
    }
}

pub fn resolve_creation_template(
    requested: Option<String>,
    format: Option<DocumentFormat>,
    defaults: bool,
) -> Result<ProjectTemplateInfo> {
    let key = match requested {
        Some(key) => key,
        None if defaults => DEFAULT_TEMPLATE_KEY.to_string(),
        None => get_doctype_from_readline(format)?,
    };
    let template = resolve_project_template(&key)?;
    if let Some(format) = format {
        if template.format != format {
            return Err(OmniDocError::Other(format!(
                "Template '{}' uses {}, but --format {} was requested",
                template.key,
                template.format.as_str(),
                format.as_str()
            )));
        }
    }
    Ok(template)
}

/// Machine-readable creation commands must never open an interactive selector.
pub fn require_explicit_creation_template(requested: Option<&str>, defaults: bool) -> Result<()> {
    if requested.is_some() || defaults {
        return Ok(());
    }
    Err(OmniDocError::Other(
        "JSON mode requires a non-interactive template choice. Use `--type <KEY>` or `--defaults`; run `omnidoc template list --json` to inspect available templates."
            .to_string(),
    ))
}

pub fn infer_title(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(|name| name.replace(['-', '_'], " "))
        .unwrap_or_else(|| "Untitled Document".to_string())
}

/// Check if omnidoc library exists
pub fn omnidoc_lib_exists() -> bool {
    GlobalConfig::load()
        .ok()
        .and_then(|config| {
            config
                .get_config()
                .and_then(|schema| schema.lib.lib.as_ref())
                .and_then(|library| library.path.as_ref())
                .map(Path::new)
                .map(Path::is_dir)
        })
        .unwrap_or(false)
}
