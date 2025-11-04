use crate::error::{OmniDocError, Result};
use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// 格式化服务
/// 提供中英文文档排版优化功能
pub struct FormatService {
    backup: bool,
    semantic: bool,
    symbol: bool,
    markdown: bool,
}

impl FormatService {
    pub fn new(backup: bool, semantic: bool, symbol: bool, markdown: bool) -> Self {
        Self {
            backup,
            semantic,
            symbol,
            markdown,
        }
    }

    /// 格式化文件
    pub fn format_file(&self, file_path: &Path) -> Result<()> {
        if !file_path.exists() {
            return Err(OmniDocError::Project(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        // 读取文件
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // 格式化每一行
        let formatted_lines: Vec<String> = lines
            .iter()
            .map(|line| self.format_line(line))
            .collect();

        // 备份文件（如果需要）
        if self.backup {
            let backup_path = format!("{}.bak", file_path.display());
            fs::copy(file_path, &backup_path)?;
        }

        // 写入格式化后的内容
        let mut output = fs::File::create(file_path)?;
        for line in formatted_lines {
            writeln!(output, "{}", line)?;
        }

        Ok(())
    }

    /// 递归格式化目录
    pub fn format_directory(&self, dir_path: &Path, extensions: &[&str]) -> Result<()> {
        use walkdir::WalkDir;

        for entry in WalkDir::new(dir_path) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                    if extensions.contains(&ext) {
                        if let Err(e) = self.format_file(path) {
                            eprintln!("Warning: Failed to format {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 格式化单行
    fn format_line(&self, line: &str) -> String {
        let mut result = line.to_string();

        // 通用格式化
        result = self.common_format(&result);

        // Markdown 特殊处理
        if self.markdown {
            result = self.md_format(&result);
        }

        // 语义格式化
        if self.semantic {
            result = self.semantic_format(&result);
        }

        // 符号格式化
        if self.symbol {
            result = self.symbol_format(&result);
        }

        result
    }

    /// 通用格式化
    fn common_format(&self, line: &str) -> String {
        let mut result = line.to_string();

        // 替换 tab 为空格
        result = result.replace('\t', "  ");

        // 中英文字符之间添加空格
        // 使用正则表达式匹配中文字符和英文字符/数字
        let re = Regex::new(r"([\p{Han}])([0-9a-zA-Z_/\\-])").unwrap();
        result = re.replace_all(&result, "$1 $2").to_string();

        let re = Regex::new(r"([0-9a-zA-Z_)\\/\\-])([\p{Han}])").unwrap();
        result = re.replace_all(&result, "$1 $2").to_string();

        // 移除中文标点符号前后的空格
        // 使用 Unicode 字符类匹配中文标点
        // 注意：正则表达式中的引号需要转义
        let punct_pattern = r#"[ \t]*([，。？！：、；…．～￥""（）「」《》——【】〈〉〔〕''])[ \t]*"#;
        let re = Regex::new(punct_pattern).unwrap();
        result = re.replace_all(&result, "$1").to_string();

        // 数字之间的连字符
        let re = Regex::new(r"(\d) *- *(\d)").unwrap();
        result = re.replace_all(&result, "$1 - $2").to_string();

        // 日期格式（YYYY-MM-DD）
        let re = Regex::new(r"([12][90]\d\d) *- *([01]\d)").unwrap();
        result = re.replace_all(&result, "$1-$2").to_string();

        result
    }

    /// Markdown 格式化
    fn md_format(&self, line: &str) -> String {
        let mut result = line.to_string();

        // \ref 前后添加空格
        let re = Regex::new(r"[ \t~]{0,5}\\ref\{([^}]{5,50})\}[ \t]*").unwrap();
        result = re.replace_all(&result, "~\\ref{$1} ").to_string();

        // \verb 处理
        let re = Regex::new(r"[ \t~]*\\verb[!|]([ 0-9a-zA-Z_/\\\-<>,.]+)[!|][ \t]*").unwrap();
        result = re.replace_all(&result, " $1 ").to_string();

        // 引用标签处理
        let re = Regex::new(r" *(\[@\w{2,4}:[\w\\-]+\]) *").unwrap();
        result = re.replace_all(&result, " $1 ").to_string();

        let re = Regex::new(r" *(\[@[\w\\-]+\]) *").unwrap();
        result = re.replace_all(&result, " $1 ").to_string();

        // 移除行首空格（在 [ 之前）
        let re = Regex::new(r"^ *\[").unwrap();
        result = re.replace_all(&result, "[").to_string();

        // ltbr div 结尾处理
        if result.contains('&') {
            let re = Regex::new(r"。$").unwrap();
            result = re.replace_all(&result, "。  ").to_string();
        }

        result
    }

    /// 语义格式化（单句换行）
    fn semantic_format(&self, line: &str) -> String {
        let mut result = line.to_string();

        // 查找行首缩进和标记
        let indent_re = Regex::new(r"^(\s*)([0-9]+\.|[\*\#\@]+|/\*+|--+)?(\s*)").unwrap();

        if let Some(caps) = indent_re.captures(&result) {
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");

            // 在句号和分号后换行（如果后面有非换行、非反斜杠、非星号、非右括号的字符）
            let re = Regex::new(r"([。；])([^\n\\\*）])").unwrap();
            let replacement = if marker.chars().any(|c| c.is_ascii_digit()) {
                // 对于编号列表，使用空格对齐而不是重复编号
                let marker_len = marker.len();
                let spaces = " ".repeat(marker_len);
                format!("$1\n{}{}{}$2", indent, spaces, after_marker)
            } else {
                format!("$1\n{}{}$2", indent, marker)
            };
            result = re.replace_all(&result, &replacement).to_string();
        } else {
            // 没有标记，使用缩进
            let indent_re = Regex::new(r"^(\s*)(.*)").unwrap();
            if let Some(caps) = indent_re.captures(&result) {
                let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let re = Regex::new(r"([。；])([^\n\\\*）])").unwrap();
                result = re.replace_all(&result, &format!("$1\n{}$2", indent)).to_string();
            }
        }

        result
    }

    /// 符号格式化（中文标点符号）
    fn symbol_format(&self, line: &str) -> String {
        let mut result = line.to_string();

        // 检查是否包含中文，且不是图片、数字列表、代码块等
        let has_chinese = Regex::new(r"[\p{Han}]").unwrap().is_match(&result);
        let is_image = Regex::new(r"^\s*!\[").unwrap().is_match(&result);
        let is_numbered_list = Regex::new(r"^\s*\d{1,}\.").unwrap().is_match(&result);
        let is_code_block = Regex::new(r"^\s*```").unwrap().is_match(&result);
        let is_comment = Regex::new(r"^\s*(/\*|\*|--|\@)").unwrap().is_match(&result);
        let is_header = Regex::new(r"^\s*\#").unwrap().is_match(&result);

        if has_chinese && !is_image && !is_numbered_list && !is_code_block && !is_comment && !is_header {
            // 替换英文标点为中文标点
            let re = Regex::new(r"([\p{Han} \w\d\\\]]{3,}), ?").unwrap();
            result = re.replace_all(&result, "$1，").to_string();

            let re = Regex::new(r"([\p{Han} \w\d\\\]]{3,})\. ?").unwrap();
            result = re.replace_all(&result, "$1。").to_string();

            result = result.replace("? ", "？");
            result = result.replace("! ", "！");
            result = result.replace(": ", "：");
            result = result.replace("; ", "；");

            // 修复误替换
            let re = Regex::new(r"(\w{2,})。c").unwrap();
            result = re.replace_all(&result, "$1.c").to_string();

            let re = Regex::new(r"([\#\@]\w{3})：").unwrap();
            result = re.replace_all(&result, "$1:").to_string();

            let re = Regex::new(r"([0-9])。([0-9])").unwrap();
            result = re.replace_all(&result, "$1.$2").to_string();
        }

        result
    }
}

