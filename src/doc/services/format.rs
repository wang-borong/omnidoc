use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// 格式化服务
/// 提供中英文文档排版优化功能
pub struct FormatService {
    backup: bool,
    semantic: bool,
    symbol: bool,
    markdown: bool,
}

// 正则表达式缓存
struct RegexCache {
    // 通用格式化
    han_to_ascii: Regex,
    ascii_to_han: Regex,
    punct_pattern: Regex,
    digit_hyphen: Regex,
    digit_equal: Regex,
    date_format: Regex,
    han_space_han: Regex,
    #[allow(dead_code)] // May be used in future versions
    math_pattern: Regex,
    code_inline_pattern: Regex,

    // TeX 格式化
    tex_verb_pattern: Regex,

    // Markdown 格式化
    ref_pattern: Regex,
    verb_pattern: Regex,
    citation_label_long: Regex,
    citation_label_short: Regex,
    header_bracket: Regex,
    period_end: Regex,
    image_pattern: Regex,
    link_pattern: Regex,

    // 语义格式化
    #[allow(dead_code)] // May be used in future versions
    indent_pattern: Regex,
    sentence_break: Regex,
    indent_simple: Regex,
    numbered_list_pattern: Regex,
    comment_line_pattern: Regex,
    c_comment_pattern: Regex,
    lua_comment_pattern: Regex,

    // 符号格式化
    has_chinese: Regex,
    is_image: Regex,
    is_numbered_list: Regex,
    is_code_block: Regex,
    is_comment: Regex,
    is_header: Regex,
    chinese_comma: Regex,
    chinese_period: Regex,
    fix_period_c: Regex,
    fix_hash_colon: Regex,
    fix_digit_period: Regex,
}

static REGEX_CACHE: OnceLock<RegexCache> = OnceLock::new();

fn get_regex_cache() -> &'static RegexCache {
    REGEX_CACHE.get_or_init(|| {
        RegexCache {
            // 通用格式化
            han_to_ascii: Regex::new(r"([\p{Han}])([=+0-9a-zA-Z_/\\-])").unwrap(),
            ascii_to_han: Regex::new(r"([=+0-9a-zA-Z_)\\/\\-])([\p{Han}])").unwrap(),
            punct_pattern: Regex::new(
                r#"[ \t]*([，。？！：、；…．～￥""（）「」《》——【】〈〉〔〕''])[ \t]*"#,
            )
            .unwrap(),
            digit_hyphen: Regex::new(r"(\d) *- *(\d)").unwrap(),
            digit_equal: Regex::new(r"(\d) *= *(\d)").unwrap(),
            date_format: Regex::new(r"([12][90]\d\d) *- *([01]\d)").unwrap(),
            // 移除中文字符之间的空格（包括中文标点）
            han_space_han: Regex::new(
                r#"([\p{Han}，。？！：、；…．～￥""（）「」《》——【】〈〉〔〕''])[ \t]+([\p{Han}，。？！：、；…．～￥""（）「」《》——【】〈〉〔〕''])"#,
            )
            .unwrap(),
            // 保护数学公式 $...$ 和 $$...$$
            math_pattern: Regex::new(r"\$+\$*([^$]+)\$+\$*").unwrap(),
            // 保护行内代码 `...`
            code_inline_pattern: Regex::new(r"`([^`]+)`").unwrap(),

            // TeX 格式化
            tex_verb_pattern: Regex::new(r"[ \t~]*\\verb[!|]([\w\-_ \t\.]{3,30})[!|][ \t]*").unwrap(),

            // Markdown 格式化
            ref_pattern: Regex::new(r"[ \t~]{0,5}\\ref\{([^}]{5,50})\}[ \t]*").unwrap(),
            verb_pattern: Regex::new(r"[ \t~]*\\verb[!|]([ 0-9a-zA-Z_/\\\-<>,.]+)[!|][ \t]*")
                .unwrap(),
            citation_label_long: Regex::new(r" *(\[@\w{2,4}:[\w\\-]+\]) *").unwrap(),
            citation_label_short: Regex::new(r" *(\[@[\w\\-]+\]) *").unwrap(),
            header_bracket: Regex::new(r"^ *\[").unwrap(),
            period_end: Regex::new(r"。$").unwrap(),
            image_pattern: Regex::new(r"!\[([^\]]*)\]\(([^)]+)\)").unwrap(),
            link_pattern: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap(),

            // 语义格式化
            indent_pattern: Regex::new(r"^(\s*)([0-9]+\.|[\*\#\@]+|/\*+|--+)?(\s*)").unwrap(),
            sentence_break: Regex::new(r"([。；])([^\n\\\*）])\s*").unwrap(),
            indent_simple: Regex::new(r"^(\s*)(.*)").unwrap(),
            numbered_list_pattern: Regex::new(r"^(\s*)([0-9]+\.)(\s*)").unwrap(),
            comment_line_pattern: Regex::new(r"^(\s*)([\*\#\@]+)(\s*)").unwrap(),
            c_comment_pattern: Regex::new(r"^(\s*)(/\*+)(\s*)").unwrap(),
            lua_comment_pattern: Regex::new(r"^(\s*)(\-\-+)(\s*)").unwrap(),

            // 符号格式化
            has_chinese: Regex::new(r"[\p{Han}]").unwrap(),
            is_image: Regex::new(r"^\s*!\[").unwrap(),
            is_numbered_list: Regex::new(r"^\s*\d{1,}\.").unwrap(),
            is_code_block: Regex::new(r"^\s*```").unwrap(),
            is_comment: Regex::new(r"^\s*(/\*|\*|--|\@)").unwrap(),
            is_header: Regex::new(r"^\s*\#").unwrap(),
            chinese_comma: Regex::new(r"([\p{Han} \w\d\\\]]{3,}), ?").unwrap(),
            chinese_period: Regex::new(r"([\p{Han} \w\d\\\]]{3,})\. ?").unwrap(),
            fix_period_c: Regex::new(r"(\w{2,})。c").unwrap(),
            fix_hash_colon: Regex::new(r"([\#\@]\w{3})：").unwrap(),
            fix_digit_period: Regex::new(r"([0-9])。([0-9])").unwrap(),
        }
    })
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
        if !fs::exists(file_path) {
            return Err(OmniDocError::Project(format!(
                "File not found: {}",
                file_path.display()
            )));
        }

        // 读取文件
        let content = fs::read_to_string(file_path)?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

        // 格式化每一行
        let formatted_lines: Vec<String> =
            lines.iter().map(|line| self.format_line(line)).collect();

        // 备份文件（如果需要）
        if self.backup {
            let backup_path = format!("{}.bak", file_path.display());
            fs::copy(file_path, &backup_path)?;
        }

        // 写入格式化后的内容
        // 注意：需要保留原始文件的换行符处理方式，并在文件末尾添加换行符
        let formatted_content = formatted_lines.join("\n");
        let mut content_bytes = formatted_content.as_bytes().to_vec();
        // 如果原始文件以换行符结尾，或者格式化后内容不为空，添加换行符
        if content.ends_with('\n') || !content_bytes.is_empty() {
            content_bytes.push(b'\n');
        }
        fs::write(file_path, &content_bytes)?;

        Ok(())
    }

    /// 递归格式化目录
    pub fn format_directory(&self, dir_path: &Path, extensions: &[&str]) -> Result<()> {
        use walkdir::WalkDir;

        for entry in WalkDir::new(dir_path) {
            let entry = entry?;
            let path = entry.path();

            if fs::is_file(path) {
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

        // TeX 格式化（总是执行）
        result = self.tex_format(&result);

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
        let cache = get_regex_cache();

        // 保护 Markdown 图片语法中的路径部分
        // 先提取图片路径，用占位符替换，格式化后再恢复
        let mut image_paths: Vec<String> = Vec::new();
        let mut link_urls: Vec<String> = Vec::new();
        let mut code_inlines: Vec<String> = Vec::new();
        let mut image_placeholder_index = 0;
        let mut link_placeholder_index = 0;
        let mut math_placeholder_index = 0;
        let mut code_placeholder_index = 0;

        // 保护数学公式 $...$ 和 $$...$$
        // 由于 Rust regex 不支持反向引用，分别处理 $ 和 $$
        // 需要保存每个公式的完整信息（包括 $ 符号数量）
        let mut math_formulas_with_type: Vec<String> = Vec::new();
        let math_pattern_double = Regex::new(r"\$\$([^$]+)\$\$").unwrap();
        let math_pattern_single = Regex::new(r"\$([^$\n]+)\$").unwrap();

        // 先处理 $$...$$（双美元符号），避免被单美元符号模式匹配
        result = math_pattern_double
            .replace_all(&result, |caps: &regex::Captures| {
                let formula = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                // 保存完整的公式
                math_formulas_with_type.push(format!("$${}$$", formula));
                // 使用占位符替换
                let placeholder = format!("$__OMNIDOC_MATH_{}__$", math_placeholder_index);
                math_placeholder_index += 1;
                placeholder
            })
            .to_string();

        // 再处理 $...$（单美元符号）
        // 排除包含占位符的情况
        result = math_pattern_single
            .replace_all(&result, |caps: &regex::Captures| {
                let formula = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                // 如果包含占位符，跳过（已经被处理过）
                if formula.contains("__OMNIDOC_MATH_") {
                    return format!("${}$", formula);
                }
                // 保存完整的公式
                math_formulas_with_type.push(format!("${}$", formula));
                // 使用占位符替换
                let placeholder = format!("$__OMNIDOC_MATH_{}__$", math_placeholder_index);
                math_placeholder_index += 1;
                placeholder
            })
            .to_string();

        // 保护行内代码 `...`
        result = cache
            .code_inline_pattern
            .replace_all(&result, |caps: &regex::Captures| {
                let code = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                // 保存原始代码
                code_inlines.push(code.to_string());
                // 使用占位符替换，格式: `__OMNIDOC_CODE_n__`
                let placeholder = format!("`__OMNIDOC_CODE_{}__`", code_placeholder_index);
                code_placeholder_index += 1;
                placeholder
            })
            .to_string();

        // 保护图片路径
        result = cache
            .image_pattern
            .replace_all(&result, |caps: &regex::Captures| {
                let alt_text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let path = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                // 保存原始路径
                image_paths.push(path.to_string());
                // 使用占位符替换，格式: ![alt](__OMNIDOC_IMG_n__)
                let placeholder = format!(
                    "![{}](__OMNIDOC_IMG_{}__)",
                    alt_text, image_placeholder_index
                );
                image_placeholder_index += 1;
                placeholder
            })
            .to_string();

        // 保护链接 URL（不包括图片链接，因为图片已经在上面处理了）
        // 使用负向前瞻确保不匹配图片链接（以 ! 开头的）
        result = cache
            .link_pattern
            .replace_all(&result, |caps: &regex::Captures| {
                let full_match = caps.get(0).map(|m| m.as_str()).unwrap_or("");
                // 如果前面有 !，说明是图片链接，跳过
                if result[..caps.get(0).unwrap().start()].ends_with('!') {
                    return full_match.to_string();
                }
                let text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let url = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                // 检查 URL 是否是占位符（已被图片处理过）
                if url.starts_with("__OMNIDOC_IMG_") {
                    return full_match.to_string();
                }
                // 保存原始 URL
                link_urls.push(url.to_string());
                // 使用占位符替换，格式: [text](__OMNIDOC_LINK_n__)
                let placeholder =
                    format!("[{}](__OMNIDOC_LINK_{}__)", text, link_placeholder_index);
                link_placeholder_index += 1;
                placeholder
            })
            .to_string();

        // 替换 tab 为空格
        result = result.replace('\t', "  ");

        // 移除中文字符之间的空格（包括中文标点）
        // 这应该在添加中英文空格之前处理
        result = cache.han_space_han.replace_all(&result, "$1$2").to_string();

        // 移除中文标点符号前后的空格
        result = cache.punct_pattern.replace_all(&result, "$1").to_string();

        // 中英文字符之间添加空格
        result = cache.han_to_ascii.replace_all(&result, "$1 $2").to_string();
        result = cache.ascii_to_han.replace_all(&result, "$1 $2").to_string();

        // 数字之间的连字符
        result = cache
            .digit_hyphen
            .replace_all(&result, "$1 - $2")
            .to_string();

        // 数字之间的等号
        result = cache
            .digit_equal
            .replace_all(&result, "$1 = $2")
            .to_string();

        // 日期格式（YYYY-MM-DD）
        result = cache.date_format.replace_all(&result, "$1-$2").to_string();

        // 恢复图片路径（不格式化路径部分）
        for (index, path) in image_paths.iter().enumerate() {
            let placeholder = format!("__OMNIDOC_IMG_{}__", index);
            result = result.replace(&placeholder, path);
        }

        // 恢复链接 URL（不格式化 URL 部分）
        for (index, url) in link_urls.iter().enumerate() {
            let placeholder = format!("__OMNIDOC_LINK_{}__", index);
            result = result.replace(&placeholder, url);
        }

        // 恢复行内代码（不格式化代码部分）
        for (index, code) in code_inlines.iter().enumerate() {
            let placeholder = format!("__OMNIDOC_CODE_{}__", index);
            result = result.replace(&placeholder, code);
        }

        // 恢复数学公式（不格式化公式部分）
        // 使用保存的公式信息恢复
        for (index, formula) in math_formulas_with_type.iter().enumerate() {
            let placeholder = format!("$__OMNIDOC_MATH_{}__$", index);
            result = result.replace(&placeholder, formula);
        }

        result
    }

    /// TeX 格式化
    fn tex_format(&self, line: &str) -> String {
        let mut result = line.to_string();
        let cache = get_regex_cache();

        // add spaces before and after verb
        result = cache
            .tex_verb_pattern
            .replace_all(&result, " \\verb!$1! ")
            .to_string();

        result
    }

    /// Markdown 格式化
    fn md_format(&self, line: &str) -> String {
        let mut result = line.to_string();
        let cache = get_regex_cache();

        // \ref 前后添加空格
        result = cache
            .ref_pattern
            .replace_all(&result, "~\\ref{$1} ")
            .to_string();

        // \verb 处理
        result = cache.verb_pattern.replace_all(&result, " $1 ").to_string();

        // 引用标签处理
        result = cache
            .citation_label_long
            .replace_all(&result, " $1 ")
            .to_string();
        result = cache
            .citation_label_short
            .replace_all(&result, " $1 ")
            .to_string();

        // 移除行首空格（在 [ 之前）
        result = cache.header_bracket.replace_all(&result, "[").to_string();

        // ltbr div 结尾处理
        if result.contains('&') {
            result = cache.period_end.replace_all(&result, "。  ").to_string();
        }

        result
    }

    /// 语义格式化（单句换行）
    fn semantic_format(&self, line: &str) -> String {
        let mut result = line.to_string();
        let cache = get_regex_cache();

        // 查找行首缩进和标记，按照 Perl 脚本的逻辑顺序匹配
        let replacement = if let Some(caps) = cache.numbered_list_pattern.captures(&result) {
            // markdown list (numbered)
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            // 对于编号列表，使用空格对齐（长度与编号相同）
            let marker_len = marker.len();
            let spaces = " ".repeat(marker_len);
            format!("$1\n{}{}{}$2", indent, spaces, after_marker)
        } else if let Some(caps) = cache.comment_line_pattern.captures(&result) {
            // comment line
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            format!("$1\n{}{}{}$2", indent, marker, after_marker)
        } else if let Some(caps) = cache.c_comment_pattern.captures(&result) {
            // c-style comment
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            format!("$1\n{}{}{}$2", indent, marker, after_marker)
        } else if let Some(caps) = cache.lua_comment_pattern.captures(&result) {
            // lua comment
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            format!("$1\n{}{}{}$2", indent, marker, after_marker)
        } else if let Some(caps) = cache.indent_simple.captures(&result) {
            // general case
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("$1\n{}$2", indent)
        } else {
            // fallback
            "$1\n$2".to_string()
        };

        // 在句号和分号后换行（如果后面有非换行、非反斜杠、非星号、非右括号的字符）
        result = cache
            .sentence_break
            .replace_all(&result, &replacement)
            .to_string();

        result
    }

    /// 符号格式化（中文标点符号）
    fn symbol_format(&self, line: &str) -> String {
        let mut result = line.to_string();
        let cache = get_regex_cache();

        // 检查是否包含中文，且不是图片、数字列表、代码块等
        let has_chinese = cache.has_chinese.is_match(&result);
        let is_image = cache.is_image.is_match(&result);
        let is_numbered_list = cache.is_numbered_list.is_match(&result);
        let is_code_block = cache.is_code_block.is_match(&result);
        let is_comment = cache.is_comment.is_match(&result);
        let is_header = cache.is_header.is_match(&result);

        if has_chinese
            && !is_image
            && !is_numbered_list
            && !is_code_block
            && !is_comment
            && !is_header
        {
            // 替换英文标点为中文标点
            result = cache.chinese_comma.replace_all(&result, "$1，").to_string();
            result = cache
                .chinese_period
                .replace_all(&result, "$1。")
                .to_string();

            result = result.replace("? ", "？");
            result = result.replace("! ", "！");
            // 替换冒号（带或不带空格）
            result = result.replace(": ", "：");
            result = result.replace(":", "：");
            // 替换分号（带或不带空格）
            result = result.replace("; ", "；");
            result = result.replace(";", "；");

            // 修复误替换
            result = cache.fix_period_c.replace_all(&result, "$1.c").to_string();
            result = cache.fix_hash_colon.replace_all(&result, "$1:").to_string();
            result = cache
                .fix_digit_period
                .replace_all(&result, "$1.$2")
                .to_string();
        }

        result
    }
}
