use crate::config::{CliOverrides, ConfigManager};
use crate::doc::services::ConverterService;
use crate::error::{OmniDocError, Result};
use std::path::{Path, PathBuf};

/// Handle the 'md2html' command
pub fn handle_md2html(inputs: Vec<String>, output: Option<String>, css: Option<String>) -> Result<()> {
    if inputs.is_empty() {
        return Err(OmniDocError::Project("No input files specified".to_string()));
    }

    // 创建配置管理器
    let cli_overrides = CliOverrides::new();
    let config_manager = ConfigManager::new(None, cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;

    // 创建转换服务
    let merged_config = config_manager.get_merged().clone();
    let converter = ConverterService::new(merged_config)?;

    // CSS 路径
    let css_path = css.as_ref().map(|s| Path::new(s));

    // 处理每个输入文件
    for input_str in &inputs {
        let input_path = Path::new(input_str);
        if !input_path.exists() {
            eprintln!("Warning: Input file not found: {}, skipping", input_str);
            continue;
        }

        // 确定输出路径
        let output_path: Option<PathBuf> = if let Some(ref out) = output {
            // 如果指定了输出，且只有一个输入，使用指定的输出
            if inputs.len() == 1 {
                Some(PathBuf::from(out))
            } else {
                // 多个输入时，如果 output 是目录，在该目录下生成
                let out_path = Path::new(out);
                if out_path.is_dir() {
                    let file_name = input_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("output");
                    Some(out_path.join(format!("{}.html", file_name)))
                } else {
                    // 多个输入但 output 不是目录，使用默认规则
                    None
                }
            }
        } else {
            None
        };

        // 执行转换
        converter.md_to_html(input_path, output_path.as_deref(), css_path)
            .map_err(|e| OmniDocError::Project(format!("Failed to convert {}: {}", input_str, e)))?;

        let final_output = if let Some(out) = output_path {
            out
        } else {
            let mut out = input_path.to_path_buf();
            out.set_extension("html");
            out
        };

        println!("✓ Converted: {} -> {}", input_str, final_output.display());
    }

    Ok(())
}

