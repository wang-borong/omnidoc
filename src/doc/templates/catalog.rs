use crate::doc::templates::generator::{is_safe_template_relative_path, list_external_templates};
use crate::doctype::{DocumentFormat, DocumentTypeRegistry};
use crate::error::{OmniDocError, Result};
use serde::Serialize;

pub const DEFAULT_TEMPLATE_KEY: &str = "ctex-md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateSource {
    BuiltIn,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectTemplateInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub format: DocumentFormat,
    pub file_name: String,
    pub source: TemplateSource,
}

pub fn list_project_templates() -> Vec<ProjectTemplateInfo> {
    let mut templates = DocumentTypeRegistry::all()
        .into_iter()
        .map(|doctype| ProjectTemplateInfo {
            key: doctype.as_str().to_string(),
            name: doctype.as_str().to_string(),
            description: doctype.description().to_string(),
            format: doctype.format(),
            file_name: doctype.file_name().to_string(),
            source: TemplateSource::BuiltIn,
        })
        .collect::<Vec<_>>();

    for template in list_external_templates() {
        let Some(format) = DocumentFormat::from_template_language(&template.language) else {
            continue;
        };
        if !is_safe_template_relative_path(&template.template_file)
            || templates
                .iter()
                .any(|existing| normalize_key(&existing.key) == normalize_key(&template.key))
        {
            continue;
        }
        let file_name = template.file_name.unwrap_or_else(|| match format {
            DocumentFormat::Markdown => "main.md".to_string(),
            DocumentFormat::Latex => "main.tex".to_string(),
        });
        if !is_safe_template_relative_path(&file_name) {
            continue;
        }
        templates.push(ProjectTemplateInfo {
            name: template.name.unwrap_or_else(|| template.key.clone()),
            key: template.key,
            description: template.description.unwrap_or_default(),
            format,
            file_name,
            source: TemplateSource::External,
        });
    }

    templates
}

pub fn resolve_project_template(input: &str) -> Result<ProjectTemplateInfo> {
    let normalized = normalize_key(input);
    let templates = list_project_templates();
    if let Some(template) = templates
        .iter()
        .find(|template| normalize_key(&template.key) == normalized)
    {
        return Ok(template.clone());
    }

    let suggestion = templates
        .iter()
        .map(|template| {
            (
                levenshtein(&normalized, &normalize_key(&template.key)),
                template.key.as_str(),
            )
        })
        .min_by_key(|(distance, _)| *distance)
        .filter(|(distance, _)| *distance <= 3)
        .map(|(_, key)| key);

    let value = match suggestion {
        Some(key) => format!("{} (did you mean '{}'?)", input.trim(), key),
        None => input.trim().to_string(),
    };
    Err(OmniDocError::UnsupportedDocumentType(value))
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut costs = (0..=right.chars().count()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut diagonal = costs[0];
        costs[0] = left_index + 1;
        for (right_index, right_char) in right.chars().enumerate() {
            let above = costs[right_index + 1];
            costs[right_index + 1] = if left_char == right_char {
                diagonal
            } else {
                1 + diagonal.min(above).min(costs[right_index])
            };
            diagonal = above;
        }
    }
    costs[right.chars().count()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_keys_are_case_and_separator_friendly() {
        let template = resolve_project_template("CTEX_MD").expect("normalized template key");
        assert_eq!(template.key, "ctex-md");
        assert_eq!(template.format, DocumentFormat::Markdown);
    }

    #[test]
    fn unsupported_template_errors_include_a_close_suggestion() {
        let error = resolve_project_template("ctex-m").expect_err("invalid template key");
        assert!(error.to_string().contains("did you mean 'ctex-md'"));
    }
}
