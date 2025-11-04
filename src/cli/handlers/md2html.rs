use crate::cli::handlers::common::create_converter_service;
use crate::error::{OmniDocError, Result};
use crate::utils::{error, path};
use std::path::Path;

/// Handle the 'md2html' command
pub fn handle_md2html(
    inputs: Vec<String>,
    output: Option<String>,
    css: Option<String>,
) -> Result<()> {
    if inputs.is_empty() {
        return Err(OmniDocError::Project(
            "No input files specified".to_string(),
        ));
    }

    let converter = create_converter_service()?;
    let css_path = css.as_ref().map(|s| Path::new(s));

    // 处理每个输入文件
    for input_str in &inputs {
        let input_path = Path::new(input_str);
        if !input_path.exists() {
            eprintln!("Warning: Input file not found: {}, skipping", input_str);
            continue;
        }

        // 确定输出路径
        let output_path: Option<std::path::PathBuf> = if let Some(ref out) = output {
            if inputs.len() == 1 {
                Some(std::path::PathBuf::from(out))
            } else {
                let out_path = Path::new(out);
                if out_path.is_dir() {
                    Some(path::determine_output_path(
                        input_path,
                        Some(out),
                        inputs.len(),
                        "html",
                    ))
                } else {
                    None
                }
            }
        } else {
            None
        };

        // 执行转换
        error::project_err(
            converter.md_to_html(
                input_path,
                output_path.as_ref().map(|p| p.as_path()),
                css_path,
            ),
            format!("Failed to convert {}", input_str),
        )?;

        let final_output =
            path::determine_output_path(input_path, output.as_deref(), inputs.len(), "html");

        println!("✓ Converted: {} -> {}", input_str, final_output.display());
    }

    Ok(())
}
