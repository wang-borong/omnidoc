use crate::doc::templates::generator::validate_external_templates;
use crate::doc::templates::{list_project_templates, TemplateSource};
use crate::error::{OmniDocError, Result};
use console::style;
use serde::Serialize;

#[derive(Serialize)]
struct TemplateValidation {
    key: String,
    source: &'static str,
    valid: bool,
    error: Option<String>,
}

pub fn handle_template_validate(key: Option<String>, json: bool) -> Result<()> {
    let requested = key.as_deref().map(normalize_key);
    let templates = list_project_templates();
    let builtin = requested.as_deref().and_then(|requested| {
        templates.iter().find(|template| {
            template.source == TemplateSource::BuiltIn && normalize_key(&template.key) == requested
        })
    });
    let builtin_keys = templates
        .iter()
        .filter(|template| template.source == TemplateSource::BuiltIn)
        .map(|template| normalize_key(&template.key))
        .collect::<Vec<_>>();

    let mut results = validate_external_templates()
        .into_iter()
        .filter(|(key, _)| {
            requested
                .as_deref()
                .is_none_or(|requested| normalize_key(key) == requested)
        })
        .map(|(key, result)| {
            let result = if builtin_keys.contains(&normalize_key(&key)) {
                Err("template key conflicts with a built-in template".to_string())
            } else {
                result
            };
            match result {
                Ok(()) => TemplateValidation {
                    key,
                    source: "external",
                    valid: true,
                    error: None,
                },
                Err(error) => TemplateValidation {
                    key,
                    source: "external",
                    valid: false,
                    error: Some(error),
                },
            }
        })
        .collect::<Vec<_>>();

    if let Some(template) = builtin {
        results.push(TemplateValidation {
            key: template.key.clone(),
            source: "built-in",
            valid: true,
            error: None,
        });
    }

    if requested.is_some() && results.is_empty() {
        return Err(OmniDocError::UnsupportedDocumentType(
            key.unwrap_or_default(),
        ));
    }

    let failed = results.iter().filter(|result| !result.valid).count();
    if json {
        let content = serde_json::to_string_pretty(&results)
            .map_err(|error| OmniDocError::Other(error.to_string()))?;
        println!("{}", content);
    } else if results.is_empty() {
        println!("{} No external templates found.", style("ℹ").cyan().bold());
    } else {
        for result in &results {
            if result.valid {
                println!(
                    "{} {} [{}]",
                    style("✔").green().bold(),
                    style(&result.key).green(),
                    result.source
                );
            } else {
                println!(
                    "{} {} — {}",
                    style("failed:").red().bold(),
                    result.key,
                    result.error.as_deref().unwrap_or("validation failed")
                );
            }
        }
        println!(
            "\n{} {} valid, {} failed.",
            style("Summary:").bold(),
            style(results.len() - failed).green(),
            style(failed).red()
        );
    }

    if failed > 0 {
        return Err(OmniDocError::Project(format!(
            "template validation failed: {failed} invalid template(s)"
        )));
    }
    Ok(())
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}
