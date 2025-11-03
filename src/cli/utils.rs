use crate::doc::templates::generator::list_external_templates;
use crate::doctype::DocumentTypeRegistry;
use crate::error::{OmniDocError, Result};
use console::style;
use inquire::Select;
use std::env;
use std::fs;
use std::path::Path;

/// Print all supported document types
pub fn print_doctypes() {
    let all = DocumentTypeRegistry::all();
    println!(
        "{} ({} types)",
        style("Current supported document types:")
            .bold()
            .underlined(),
        all.len()
    );
    println!("{}", DocumentTypeRegistry::list_display());
    let externals = list_external_templates();
    if !externals.is_empty() {
        println!(
            "\n{} ({} templates)",
            style("External templates:").bold().underlined(),
            externals.len()
        );
        for t in externals {
            let name = t.name.clone().unwrap_or_else(|| t.key.clone());
            match t.description {
                Some(desc) if !desc.is_empty() => println!("- {} — {} ({})", t.key, name, desc),
                _ => println!("- {} — {}", t.key, name),
            }
        }
    }
    println!(
        "{} {}",
        style("ℹ").cyan().bold(),
        style("Use arrow keys to navigate, Enter to select").cyan()
    );
}

/// Get document type from readline with cleanup on error
pub fn get_doctype_from_readline<O, N>(orig_path: O, path: N) -> Result<String>
where
    O: AsRef<Path>,
    N: AsRef<Path>,
{
    let mut items: Vec<String> = DocumentTypeRegistry::all()
        .into_iter()
        .map(|dt| format!("{} — {}", dt.as_str(), dt.description()))
        .collect();
    items.push("[Cancel] 取消".to_string());

    let selection = Select::new("请选择文档类型:", items)
        .with_page_size(10)
        .with_help_message("上下键选择，Enter 确认，Esc/Ctrl+C 取消")
        .prompt();

    let selection = match selection {
        Ok(sel) => sel,
        Err(
            inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted,
        ) => {
            let _ = env::set_current_dir(orig_path.as_ref());
            let _ = fs::remove_dir_all(path.as_ref());
            return Err(OmniDocError::Other("操作已取消".to_string()));
        }
        Err(e) => {
            let _ = env::set_current_dir(orig_path.as_ref());
            let _ = fs::remove_dir_all(path.as_ref());
            return Err(OmniDocError::Other(format!("Prompt failed ({})", e)));
        }
    };

    if selection.starts_with("[Cancel]") {
        let _ = env::set_current_dir(orig_path.as_ref());
        let _ = fs::remove_dir_all(path.as_ref());
        return Err(OmniDocError::Other("操作已取消".to_string()));
    }

    // selection is in the form "key — desc"; split to return the key
    let key = selection.split(' ').next().unwrap_or("").to_string();

    Ok(key)
}

/// Check if omnidoc library exists
pub fn omnidoc_lib_exists() -> bool {
    let local_data_dir = match dirs::data_local_dir() {
        Some(dir) => dir,
        None => return false,
    };
    let omnidoc_lib_dir = local_data_dir.join("omnidoc");
    omnidoc_lib_dir.exists()
}
