use crate::doc::services::FormatService;
use crate::error::{OmniDocError, Result};
use std::path::Path;

/// Handle the 'fmt' command
pub fn handle_fmt(paths: Vec<String>, backup: bool, semantic: bool, symbol: bool) -> Result<()> {
    // 创建格式化服务
    // 检测是否为 markdown 文件（通过扩展名）
    let format_service = FormatService::new(backup, semantic, symbol, true);

    if paths.is_empty() {
        // 如果没有指定路径，格式化当前目录
        let current_dir = std::env::current_dir()
            .map_err(|e| OmniDocError::Io(e))?;
        
        format_service.format_directory(&current_dir, &["md", "tex"])?;
        println!("✓ Formatted all markdown and latex files in current directory");
    } else {
        // 处理指定的路径
        for path_str in &paths {
            let path = Path::new(path_str);
            
            if !path.exists() {
                eprintln!("Warning: Path not found: {}, skipping", path_str);
                continue;
            }

            if path.is_file() {
                // 格式化单个文件
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if ext == "md" || ext == "tex" {
                        format_service.format_file(path)?;
                        println!("✓ Formatted: {}", path_str);
                    } else {
                        eprintln!("Warning: Unsupported file type: {}, skipping", path_str);
                    }
                }
            } else if path.is_dir() {
                // 递归格式化目录
                format_service.format_directory(path, &["md", "tex"])?;
                println!("✓ Formatted all markdown and latex files in: {}", path_str);
            } else {
                eprintln!("Warning: Invalid path: {}, skipping", path_str);
            }
        }
    }

    Ok(())
}

