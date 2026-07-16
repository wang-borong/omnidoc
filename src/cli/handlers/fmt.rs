use crate::doc::services::FormatService;
use crate::error::{OmniDocError, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Handle the 'fmt' command
pub fn handle_fmt(
    paths: Vec<String>,
    backup: bool,
    check: bool,
    diff: bool,
    semantic: bool,
    symbol: bool,
) -> Result<()> {
    let format_service = FormatService::new(backup, semantic, symbol, true);
    let roots = if paths.is_empty() {
        vec![std::env::current_dir().map_err(OmniDocError::Io)?]
    } else {
        paths.iter().map(Path::new).map(Path::to_path_buf).collect()
    };
    let mut files = Vec::new();
    for root in roots {
        if !root.exists() {
            return Err(OmniDocError::Project(format!(
                "Path not found: {}",
                root.display()
            )));
        }
        if root.is_file() {
            if supported_file(&root) {
                files.push(root);
            } else {
                return Err(OmniDocError::Project(format!(
                    "Unsupported file type: {}",
                    root.display()
                )));
            }
            continue;
        }
        for entry in WalkDir::new(&root) {
            let entry = entry?;
            if entry.file_type().is_file() && supported_file(entry.path()) {
                files.push(entry.into_path());
            }
        }
    }
    files.sort();
    files.dedup();

    let mut changed = Vec::new();
    for file in files {
        if diff {
            if let Some(output) = format_service.unified_diff(&file)? {
                print!("{output}");
                changed.push(file);
            }
        } else if check {
            if format_service.would_change(&file)? {
                println!("would format {}", file.display());
                changed.push(file);
            }
        } else if format_service.format_file(&file)? {
            println!("formatted {}", file.display());
            changed.push(file);
        }
    }

    if (check || diff) && !changed.is_empty() {
        return Err(OmniDocError::Project(format!(
            "{} file(s) require formatting",
            changed.len()
        )));
    }

    if !check && !diff {
        println!("format complete: {} file(s) changed", changed.len());
    }

    Ok(())
}

fn supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "md" | "markdown" | "mdown" | "tex"))
}
