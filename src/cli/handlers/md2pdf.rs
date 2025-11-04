use crate::config::{CliOverrides, ConfigManager};
use crate::doc::services::ConverterService;
use crate::error::{OmniDocError, Result};
use std::path::Path;

/// Handle the 'md2pdf' command
pub fn handle_md2pdf(inputs: Vec<String>, output: Option<String>) -> Result<()> {
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

    // 处理每个输入文件
    for input_str in &inputs {
        let input_path = Path::new(input_str);
        if !input_path.exists() {
            eprintln!("Warning: Input file not found: {}, skipping", input_str);
            continue;
        }

        // 确定输出路径
        let output_path = if let Some(ref out) = output {
            // 如果指定了输出，且只有一个输入，使用指定的输出
            if inputs.len() == 1 {
                Some(Path::new(out))
            } else {
                // 多个输入时，忽略 output 参数，使用默认规则
                None
            }
        } else {
            None
        };

        // 执行转换
        converter.md_to_pdf(input_path, output_path)
            .map_err(|e| OmniDocError::Project(format!("Failed to convert {}: {}", input_str, e)))?;

        let final_output = if let Some(out) = output_path {
            out.to_path_buf()
        } else {
            let mut out = input_path.to_path_buf();
            out.set_extension("pdf");
            out
        };

        println!("✓ Converted: {} -> {}", input_str, final_output.display());
    }

    Ok(())
}

