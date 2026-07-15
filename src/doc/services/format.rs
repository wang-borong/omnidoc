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
    default_markdown: bool,
}

// 正则表达式缓存
struct RegexCache {
    // 通用格式化
    han_to_ascii: Regex,
    ascii_to_han: Regex,
    punct_pattern: Regex,
    digit_equal: Regex,
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
    reference_definition: Regex,
    table_delimiter: Regex,

    // 语义格式化
    #[allow(dead_code)] // May be used in future versions
    indent_pattern: Regex,
    sentence_break: Regex,
    indent_simple: Regex,
    numbered_list_pattern: Regex,
    bullet_list_pattern: Regex,
    blockquote_pattern: Regex,
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
            digit_equal: Regex::new(r"(\d) *= *(\d)").unwrap(),
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
            reference_definition: Regex::new(r"^\[[^\]]+\]:\s*\S+").unwrap(),
            table_delimiter: Regex::new(
                r"^\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?$",
            )
            .unwrap(),

            // 语义格式化
            indent_pattern: Regex::new(r"^(\s*)([0-9]+\.|[\*\#\@]+|/\*+|--+)?(\s*)").unwrap(),
            sentence_break: Regex::new(r"([。；])([^\n\\\*）])\s*").unwrap(),
            indent_simple: Regex::new(r"^(\s*)(.*)").unwrap(),
            numbered_list_pattern: Regex::new(r"^(\s*)([0-9]+\.)(\s*)").unwrap(),
            bullet_list_pattern: Regex::new(r"^(\s*)([-+*])(\s+(?:\[[ xX]\]\s+)?)").unwrap(),
            blockquote_pattern: Regex::new(r"^(\s*(?:>\s*)+)").unwrap(),
            comment_line_pattern: Regex::new(r"^(\s*)([\*\@]+)(\s+)").unwrap(),
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

#[derive(Debug, Clone)]
enum ProtectedBlock {
    FrontMatter(String),
    Fence { marker: u8, length: usize },
    Math(&'static str),
    HtmlComment,
    HtmlElement(String),
    LatexEnvironment(String),
    SingleLine,
}

impl ProtectedBlock {
    fn opening(trimmed: &str) -> Option<Self> {
        let bytes = trimmed.as_bytes();
        if let Some(marker) = bytes
            .first()
            .copied()
            .filter(|byte| matches!(byte, b'`' | b'~'))
        {
            let length = bytes.iter().take_while(|byte| **byte == marker).count();
            if length >= 3 {
                return Some(Self::Fence { marker, length });
            }
        }
        if trimmed == "$$" {
            return Some(Self::Math("$$"));
        }
        if trimmed == r"\[" {
            return Some(Self::Math(r"\]"));
        }
        if trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() > 4 {
            return Some(Self::SingleLine);
        }
        if trimmed.starts_with("<!--") {
            return Some(if trimmed.contains("-->") {
                Self::SingleLine
            } else {
                Self::HtmlComment
            });
        }
        if let Some(environment) = latex_environment(trimmed) {
            let closing = format!(r"\end{{{}}}", environment);
            return Some(if trimmed.contains(&closing) {
                Self::SingleLine
            } else {
                Self::LatexEnvironment(environment)
            });
        }
        if let Some(tag) = raw_html_container(trimmed) {
            let closing = format!("</{}>", tag);
            return Some(if trimmed.to_ascii_lowercase().contains(&closing) {
                Self::SingleLine
            } else {
                Self::HtmlElement(tag)
            });
        }
        trimmed.starts_with('<').then_some(Self::SingleLine)
    }

    fn requires_closing(&self) -> bool {
        !matches!(self, Self::SingleLine)
    }

    fn closes(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
        match self {
            Self::FrontMatter(marker) => {
                line.trim() == marker || (marker == "---" && line.trim() == "...")
            }
            Self::Fence { marker, length } => {
                let bytes = trimmed.as_bytes();
                bytes.first() == Some(marker)
                    && bytes.iter().take_while(|byte| **byte == *marker).count() >= *length
            }
            Self::Math(marker) => line.trim() == *marker,
            Self::HtmlComment => line.contains("-->"),
            Self::HtmlElement(tag) => trimmed
                .to_ascii_lowercase()
                .contains(&format!("</{}>", tag)),
            Self::LatexEnvironment(environment) => {
                line.contains(&format!(r"\end{{{}}}", environment))
            }
            Self::SingleLine => true,
        }
    }
}

fn latex_environment(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix(r"\begin{")?;
    let end = rest.find('}')?;
    let environment = &rest[..end];
    (!environment.is_empty()).then(|| environment.to_string())
}

fn raw_html_container(trimmed: &str) -> Option<String> {
    let lower = trimmed.to_ascii_lowercase();
    [
        "script", "style", "pre", "code", "table", "div", "details", "summary", "math", "svg",
    ]
    .into_iter()
    .find(|tag| lower.starts_with(&format!("<{}", tag)))
    .map(str::to_string)
}

fn is_structural_markdown_line(line: &str, markdown: bool) -> bool {
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return true;
    }
    if !markdown {
        return trimmed.starts_with('%') || trimmed.starts_with('\\');
    }
    if line.starts_with("    ") || line.starts_with('\t') {
        return true;
    }
    let cache = get_regex_cache();
    if trimmed.starts_with(":::")
        || (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || cache.reference_definition.is_match(trimmed)
    {
        return true;
    }
    let pipe_count = trimmed
        .chars()
        .filter(|character| *character == '|')
        .count();
    if pipe_count > 0 && (trimmed.starts_with('|') || trimmed.ends_with('|')) {
        return true;
    }
    cache.table_delimiter.is_match(trimmed)
}

fn protect_inline_tokens(line: &str) -> (String, Vec<String>) {
    let bytes = line.as_bytes();
    let mut masked = String::with_capacity(line.len());
    let mut protected = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let end = match bytes[index] {
            b'\\' => bytes
                .get(index + 1)
                .copied()
                .filter(u8::is_ascii_punctuation)
                .map(|_| index + 2),
            b'`' | b'$' => {
                let marker = bytes[index];
                let run = bytes[index..]
                    .iter()
                    .take_while(|byte| **byte == marker)
                    .count();
                find_closing_run(bytes, index + run, marker, run).map(|closing| closing + run)
            }
            b'<' => bytes[index + 1..]
                .iter()
                .position(|byte| *byte == b'>')
                .map(|offset| index + offset + 2),
            _ => None,
        };
        if let Some(end) = end {
            let token = line[index..end].to_string();
            let placeholder = format!("\u{e000}{}\u{e001}", protected.len());
            protected.push(token);
            masked.push_str(&placeholder);
            index = end;
            continue;
        }
        let character = line[index..].chars().next().expect("character boundary");
        masked.push(character);
        index += character.len_utf8();
    }
    (masked, protected)
}

fn find_closing_run(bytes: &[u8], mut index: usize, marker: u8, length: usize) -> Option<usize> {
    while index < bytes.len() {
        if bytes[index] == marker {
            let run = bytes[index..]
                .iter()
                .take_while(|byte| **byte == marker)
                .count();
            if run == length {
                return Some(index);
            }
            index += run;
        } else {
            index += 1;
        }
    }
    None
}

fn restore_inline_tokens(mut line: String, protected: &[String]) -> String {
    for (index, token) in protected.iter().enumerate() {
        line = line.replace(&format!("\u{e000}{}\u{e001}", index), token);
    }
    line
}

impl FormatService {
    pub fn new(backup: bool, semantic: bool, symbol: bool, markdown: bool) -> Self {
        Self {
            backup,
            semantic,
            symbol,
            default_markdown: markdown,
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
        let markdown = file_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| matches!(extension, "md" | "markdown" | "mdown"))
            .unwrap_or(self.default_markdown);
        let formatted_content = self.format_content_with_mode(&content, markdown);

        // 备份文件（如果需要）
        if self.backup {
            let backup_path = format!("{}.bak", file_path.display());
            fs::copy(file_path, &backup_path)?;
        }

        // 写入格式化后的内容
        // 注意：需要保留原始文件的换行符处理方式，并在文件末尾添加换行符
        let mut content_bytes = formatted_content.as_bytes().to_vec();
        // 如果原始文件以换行符结尾，或者格式化后内容不为空，添加换行符
        if content.ends_with('\n') || !content_bytes.is_empty() {
            content_bytes.push(b'\n');
        }
        fs::write(file_path, &content_bytes)?;

        Ok(())
    }

    /// Format Markdown content while preserving structural regions that must be byte-stable.
    #[cfg(test)]
    fn format_content(&self, content: &str) -> String {
        self.format_content_with_mode(content, self.default_markdown)
    }

    fn format_content_with_mode(&self, content: &str, markdown: bool) -> String {
        let mut formatted_lines = Vec::new();
        let mut protected_block = None::<ProtectedBlock>;

        for (index, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();

            if index == 0 && matches!(line.trim(), "---" | "+++") {
                protected_block = Some(ProtectedBlock::FrontMatter(line.trim().to_string()));
                formatted_lines.push(line.to_string());
                continue;
            }
            if let Some(block) = &protected_block {
                formatted_lines.push(line.to_string());
                if block.closes(line) {
                    protected_block = None;
                }
                continue;
            }

            if let Some(block) = ProtectedBlock::opening(trimmed) {
                formatted_lines.push(line.to_string());
                protected_block = block.requires_closing().then_some(block);
                continue;
            }
            if is_structural_markdown_line(line, markdown) {
                formatted_lines.push(line.to_string());
                continue;
            }

            formatted_lines.push(self.format_line(line, markdown));
        }

        formatted_lines.join("\n")
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
    fn format_line(&self, line: &str, markdown: bool) -> String {
        let (mut result, protected) = protect_inline_tokens(line);

        // 通用格式化
        result = self.common_format(&result);

        // TeX 格式化（总是执行）
        result = self.tex_format(&result);

        // Markdown 特殊处理
        if markdown {
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

        restore_inline_tokens(result, &protected)
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

        // 数字之间的等号
        result = cache
            .digit_equal
            .replace_all(&result, "$1 = $2")
            .to_string();

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

        let trimmed = result.trim_start();
        if trimmed.starts_with('#')
            || trimmed.starts_with('|')
            || trimmed.starts_with("```")
            || trimmed.starts_with("~~~")
            || trimmed == "---"
            || trimmed == "***"
            || trimmed.starts_with('<')
        {
            return result;
        }

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
        } else if let Some(caps) = cache.bullet_list_pattern.captures(&result) {
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let spaces = " ".repeat(marker.chars().count() + after_marker.chars().count());
            format!("$1\n{}{}$2", indent, spaces)
        } else if let Some(caps) = cache.blockquote_pattern.captures(&result) {
            let prefix = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            format!("$1\n{}$2", prefix)
        } else if let Some(caps) = cache.comment_line_pattern.captures(&result) {
            // Markdown bullet/comment continuation: keep the marker only on the first line.
            let indent = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("");
            let after_marker = caps.get(3).map(|m| m.as_str()).unwrap_or("");
            let spaces = " ".repeat(marker.chars().count() + after_marker.chars().count());
            format!("$1\n{}{}$2", indent, spaces)
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

#[cfg(test)]
mod tests {
    use super::FormatService;

    #[test]
    fn preserves_front_matter_fenced_code_dates_and_leading_emphasis() {
        let service = FormatService::new(false, true, true, true);
        let input = concat!(
            "---\n",
            "title: 中文标题\n",
            "date: 2026-07-14 15:38:13\n",
            "---\n\n",
            "**本书的范围。** 第一项；第二项。\n\n",
            "```yaml\n",
            "title: 中文标题\n",
            "date: 2026-07-14\n",
            "```\n",
        );

        let output = service.format_content(input);

        assert!(output.starts_with("---\ntitle: 中文标题\ndate: 2026-07-14 15:38:13\n---\n"));
        assert!(output.contains("**本书的范围。** 第一项；\n第二项。"));
        assert!(output.contains("```yaml\ntitle: 中文标题\ndate: 2026-07-14\n```"));
        assert!(!output.contains("**第二项"));
    }

    #[test]
    fn semantic_format_does_not_turn_sentence_continuations_into_new_bullets() {
        let service = FormatService::new(false, true, false, true);
        let output = service
            .format_content("* 第一项；第二句。\n- 第二项；续句。\n> 引用第一句；引用第二句。\n");

        assert_eq!(
            output,
            "* 第一项；\n  第二句。\n- 第二项；\n  续句。\n> 引用第一句；\n> 引用第二句。"
        );
    }

    #[test]
    fn preserves_markdown_blocks_and_inline_tokens() {
        let service = FormatService::new(false, true, true, true);
        let input = concat!(
            "+++\n",
            "title = \"中文ABC:原样\"\n",
            "+++\n\n",
            "$$\n",
            "中文ABC=a+b, raw\n",
            "$$\n\n",
            "<table>\n",
            "<tr><td>中文ABC: raw</td></tr>\n",
            "</table>\n\n",
            "| 中文ABC, | value:raw |\n",
            "|---|---|\n",
            "[手册]: https://example.com/a:b?q=中文\n\n",
            "正文中文ABC，``code:中文ABC`` 与 $a=b$ 以及 <span data-x=\"a:b\">内容</span>；参见 \\ref{chapter-one} 和转义 \\*。\n",
        );

        let output = service.format_content(input);

        assert!(output.contains("title = \"中文ABC:原样\""));
        assert!(output.contains("$$\n中文ABC=a+b, raw\n$$"));
        assert!(output.contains("<tr><td>中文ABC: raw</td></tr>"));
        assert!(output.contains("| 中文ABC, | value:raw |\n|---|---|"));
        assert!(output.contains("[手册]: https://example.com/a:b?q=中文"));
        assert!(output.contains("``code:中文ABC``"));
        assert!(output.contains("$a=b$"));
        assert!(output.contains("<span data-x=\"a:b\">"));
        assert!(output.contains("\\ref{chapter-one}"));
        assert!(output.contains("\\*"));
    }

    #[test]
    fn conservative_formatting_is_idempotent() {
        let service = FormatService::new(false, false, false, true);
        let once = service.format_content("中文ABC 与 `x=y`。\n\n普通文本DEF。\n");
        let twice = service.format_content(&once);

        assert_eq!(once, twice);
        assert!(once.contains("中文 ABC"));
        assert!(once.contains("`x=y`"));
    }

    #[test]
    fn tex_mode_preserves_command_and_environment_lines() {
        let service = FormatService::new(false, true, true, true);
        let input = concat!(
            "\\newcommand{\\BookName}{中文ABC:原样}\n",
            "\\begin{align}\n",
            "中文ABC &= a+b, raw\\\\\n",
            "\\end{align}\n",
            "正文中文ABC。\n",
        );

        let output = service.format_content_with_mode(input, false);

        assert!(output.contains("\\newcommand{\\BookName}{中文ABC:原样}"));
        assert!(output.contains("\\begin{align}\n中文ABC &= a+b, raw\\\\\n\\end{align}"));
        assert!(output.ends_with("正文中文 ABC。"));
    }
}
