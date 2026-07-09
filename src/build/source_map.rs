use crate::build::executor::BuildExecutor;
use crate::error::Result;
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const MAX_NEEDLES: usize = 8;
const CONTEXT_RADIUS: usize = 1;
const MAX_CONTEXT_CHARS: usize = 140;

#[derive(Debug, Clone)]
struct SourceSpan {
    file: PathBuf,
    line: usize,
    column: usize,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticContextLine {
    pub line: usize,
    pub text: String,
    pub primary: bool,
    pub marker_column: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDiagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub kind: String,
    pub snippet: String,
    pub message: String,
    pub context: Vec<DiagnosticContextLine>,
    pub help: Option<String>,
}

impl MarkdownDiagnostic {
    pub fn render(&self) -> String {
        let mut rendered = format!(
            "Markdown source diagnostic: {}:{}:{}: {}",
            self.file, self.line, self.column, self.kind
        );

        if !self.context.is_empty() {
            let width = self
                .context
                .iter()
                .map(|line| line.line.to_string().len())
                .max()
                .unwrap_or(1);
            rendered.push_str("\n  |");
            for line in &self.context {
                rendered.push_str(&format!(
                    "\n{:>width$} | {}",
                    line.line,
                    line.text,
                    width = width
                ));
                if line.primary {
                    let marker_column = line.marker_column.unwrap_or(self.column).max(1);
                    let marker_padding = " ".repeat(marker_column.saturating_sub(1));
                    rendered.push_str(&format!(
                        "\n{:>width$} | {}^",
                        "",
                        marker_padding,
                        width = width
                    ));
                }
            }
        } else if !self.snippet.is_empty() {
            rendered.push_str(&format!("\n  {}", self.snippet));
        }

        rendered.push_str(&format!("\n  note: {}", self.message));
        if let Some(help) = &self.help {
            rendered.push_str(&format!("\n  help: {}", help));
        }
        rendered
    }
}

pub fn locate_markdown_error(
    executor: &BuildExecutor,
    entry_file: &Path,
    diagnostic: &str,
) -> Option<String> {
    locate_markdown_diagnostic(executor, entry_file, diagnostic)
        .map(|diagnostic| diagnostic.render())
}

pub fn locate_markdown_diagnostic(
    executor: &BuildExecutor,
    entry_file: &Path,
    diagnostic: &str,
) -> Option<MarkdownDiagnostic> {
    if let Some(diagnostic) = locate_direct_markdown_location(entry_file, diagnostic) {
        return Some(diagnostic);
    }

    let needles = extract_needles(diagnostic);
    if needles.is_empty() {
        return None;
    }

    if let Some(line_hint) = locate_in_raw_markdown(entry_file, diagnostic, &needles) {
        return Some(line_hint);
    }

    let spans = load_source_spans(executor, entry_file).ok()?;
    for needle in &needles {
        let needle = normalize_needle(needle);
        if !is_searchable_needle(&needle) {
            continue;
        }

        if let Some(span) = spans.iter().find(|span| {
            span.text
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
        }) {
            return Some(build_markdown_diagnostic(
                &span.file,
                span.line,
                span.column,
                diagnostic,
                &span.text,
            ));
        }
    }

    None
}

fn load_source_spans(executor: &BuildExecutor, entry_file: &Path) -> Result<Vec<SourceSpan>> {
    let input = entry_file.to_string_lossy().to_string();
    let args = ["-f", "commonmark_x+sourcepos", "-t", "json", input.as_str()];
    let json = executor.execute_with_output("pandoc", &args)?;
    let value: Value = serde_json::from_str(&json)
        .map_err(|e| crate::error::OmniDocError::Other(e.to_string()))?;
    let mut spans = Vec::new();
    collect_spans(entry_file, &value, &mut spans);
    Ok(spans)
}

fn collect_spans(source_file: &Path, value: &Value, spans: &mut Vec<SourceSpan>) {
    if let Some((line, column)) = data_pos_start(value) {
        let text = collect_text(value);
        if !text.trim().is_empty() {
            spans.push(SourceSpan {
                file: source_file.to_path_buf(),
                line,
                column,
                text,
            });
        }
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_spans(source_file, item, spans);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_spans(source_file, item, spans);
            }
        }
        _ => {}
    }
}

fn data_pos_start(value: &Value) -> Option<(usize, usize)> {
    let attrs = find_attr_keyvals(value)?;
    for keyval in attrs {
        let pair = keyval.as_array()?;
        let key = pair.first()?.as_str()?;
        let data_pos = pair.get(1)?.as_str()?;
        if key == "data-pos" {
            return parse_data_pos_start(data_pos);
        }
    }
    None
}

fn parse_data_pos_start(data_pos: &str) -> Option<(usize, usize)> {
    let start = data_pos.split('-').next().unwrap_or(data_pos);
    let mut parts = start.split(':');
    let line = parts.next()?.parse::<usize>().ok()?;
    let column = parts
        .next()
        .and_then(|part| part.parse::<usize>().ok())
        .unwrap_or(1);
    Some((line, column))
}

fn find_attr_keyvals(value: &Value) -> Option<&Vec<Value>> {
    match value {
        Value::Array(items) if items.len() == 3 => items.get(2)?.as_array(),
        Value::Array(items) => {
            for item in items {
                if let Some(attrs) = find_attr_keyvals(item) {
                    return Some(attrs);
                }
            }
            None
        }
        Value::Object(map) => map.get("c").and_then(find_attr_keyvals),
        _ => None,
    }
}

fn collect_text(value: &Value) -> String {
    let mut parts = Vec::new();
    collect_text_parts(value, &mut parts);
    parts.join(" ")
}

fn collect_text_parts(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(tag) = map.get("t").and_then(Value::as_str) {
                match tag {
                    "Str" | "Code" | "Math" | "RawInline" | "RawBlock" => {
                        collect_text_parts(map.get("c").unwrap_or(&Value::Null), parts);
                        return;
                    }
                    "Space" | "SoftBreak" | "LineBreak" => {
                        parts.push(" ".to_string());
                        return;
                    }
                    _ => {}
                }
            }

            for item in map.values() {
                collect_text_parts(item, parts);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_text_parts(item, parts);
            }
        }
        Value::String(text) => {
            if !text.trim().is_empty() {
                parts.push(text.clone());
            }
        }
        _ => {}
    }
}

fn locate_in_raw_markdown(
    entry_file: &Path,
    diagnostic: &str,
    needles: &[String],
) -> Option<MarkdownDiagnostic> {
    let candidates = markdown_candidates(entry_file);
    for needle in needles {
        let needle = normalize_needle(needle);
        if !is_searchable_needle(&needle) {
            continue;
        }

        for candidate in &candidates {
            let Ok(content) = std::fs::read_to_string(candidate) else {
                continue;
            };
            for (index, line) in content.lines().enumerate() {
                if let Some(column) = find_column(line, &needle) {
                    return Some(build_markdown_diagnostic(
                        candidate,
                        index + 1,
                        column,
                        diagnostic,
                        line,
                    ));
                }
            }
        }
    }
    None
}

fn locate_direct_markdown_location(
    entry_file: &Path,
    diagnostic: &str,
) -> Option<MarkdownDiagnostic> {
    let location_re = Regex::new(
        r#"(?m)(?P<file>(?:[A-Za-z]:[\\/])?[^:'"`\n]+?\.m(?:d|arkdown)):(?P<line>\d+)(?::(?P<column>\d+))?"#,
    )
    .expect("location regex");
    for capture in location_re.captures_iter(diagnostic) {
        let file = capture.name("file")?.as_str().trim();
        let line = capture.name("line")?.as_str().parse::<usize>().ok()?;
        let column = capture
            .name("column")
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .unwrap_or(1);
        let Some(source_file) = resolve_diagnostic_markdown_path(entry_file, file) else {
            continue;
        };
        let source_content = std::fs::read_to_string(&source_file).ok()?;
        let snippet = source_content
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("");
        return Some(build_markdown_diagnostic(
            &source_file,
            line,
            column,
            diagnostic,
            snippet,
        ));
    }

    let line_column_re = Regex::new(
        r"(?i)(?:at\s+)?line\s+(?P<line>\d+)(?:\s*[,;:]\s*|\s+)(?:column|col)\s+(?P<column>\d+)",
    )
    .expect("line column regex");
    if let Some(capture) = line_column_re.captures_iter(diagnostic).next() {
        let line = capture.name("line")?.as_str().parse::<usize>().ok()?;
        let column = capture
            .name("column")
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .unwrap_or(1);
        let source_content = std::fs::read_to_string(entry_file).ok()?;
        let snippet = source_content
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("");
        return Some(build_markdown_diagnostic(
            entry_file, line, column, diagnostic, snippet,
        ));
    }

    None
}

fn resolve_diagnostic_markdown_path(entry_file: &Path, diagnostic_path: &str) -> Option<PathBuf> {
    let path = Path::new(diagnostic_path);
    if path.is_absolute() {
        if path.exists() {
            return Some(path.to_path_buf());
        }

        return markdown_candidates(entry_file)
            .into_iter()
            .find(|candidate| path_suffix_matches(candidate, diagnostic_path));
    }

    let root = project_root(entry_file);
    let relative_to_root = root.join(path);
    if relative_to_root.exists() {
        return Some(relative_to_root);
    }

    markdown_candidates(entry_file)
        .into_iter()
        .find(|candidate| path_suffix_matches(candidate, diagnostic_path))
}

fn path_suffix_matches(candidate: &Path, diagnostic_path: &str) -> bool {
    let normalized_candidate = candidate.to_string_lossy().replace('\\', "/");
    let normalized_diagnostic = diagnostic_path.replace('\\', "/");
    let diagnostic_file_name = Path::new(diagnostic_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(diagnostic_path);
    normalized_candidate.ends_with(&normalized_diagnostic)
        || candidate
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name == diagnostic_file_name)
            .unwrap_or(false)
}

fn find_column(line: &str, needle: &str) -> Option<usize> {
    if let Some(byte_index) = line.find(needle) {
        return Some(line[..byte_index].chars().count() + 1);
    }

    let lower_line = line.to_ascii_lowercase();
    let lower_needle = needle.to_ascii_lowercase();
    lower_line
        .find(&lower_needle)
        .map(|byte_index| line[..byte_index].chars().count() + 1)
}

fn extract_needles(diagnostic: &str) -> Vec<String> {
    let mut needles = Vec::new();
    let macro_re = Regex::new(r"\\[A-Za-z@]+").expect("macro regex");
    let quoted_re = Regex::new(r#"[`'"]([^`'"]{3,80})[`'"]"#).expect("quoted regex");
    let citation_re = Regex::new(r"@[-_:.#A-Za-z0-9]+").expect("citation regex");
    let resource_re =
        Regex::new(r#"(?i)(?:pandoc:\s*)?([^\s:'"`]+?\.(?:png|jpg|jpeg|gif|webp|svg|pdf|eps|bib|csl|md|markdown|tex|sty|cls|bst|bbx|cbx|lua|csv|tsv|yaml|yml|json))"#)
            .expect("resource regex");
    let latex_file_re = Regex::new(
        r#"(?i)File [`'"]?([^`'"\s]+?\.(?:sty|cls|tex|bib|bst|bbx|cbx))[`'"]? not found"#,
    )
    .expect("latex file regex");
    let unicode_re =
        Regex::new(r"(?i)Unicode character\s+(.+?)\s+\(U\+[0-9A-F]+\)").expect("unicode regex");
    let latex_line_re = Regex::new(r"(?m)^l\.\d+\s*(?P<line>.*)$").expect("latex line regex");

    for capture in macro_re.find_iter(diagnostic) {
        push_needle(&mut needles, capture.as_str());
    }
    for capture in citation_re.find_iter(diagnostic) {
        push_needle(&mut needles, capture.as_str());
    }
    for capture in quoted_re.captures_iter(diagnostic) {
        if let Some(value) = capture.get(1) {
            push_needle(&mut needles, value.as_str());
        }
    }
    for capture in resource_re.captures_iter(diagnostic) {
        if let Some(value) = capture.get(1) {
            push_needle(&mut needles, value.as_str());
        }
    }
    for capture in latex_file_re.captures_iter(diagnostic) {
        if let Some(value) = capture.get(1) {
            push_needle(&mut needles, value.as_str());
            if let Some(stem) = Path::new(value.as_str())
                .file_stem()
                .and_then(|stem| stem.to_str())
            {
                push_needle(&mut needles, stem);
            }
        }
    }
    for capture in unicode_re.captures_iter(diagnostic) {
        if let Some(value) = capture.get(1) {
            push_needle(&mut needles, value.as_str());
        }
    }
    for capture in latex_line_re.captures_iter(diagnostic) {
        let Some(line) = capture.name("line").map(|line| line.as_str().trim()) else {
            continue;
        };
        push_needle(&mut needles, line);
        for word in line.split(|ch: char| ch.is_whitespace() || ch == '{' || ch == '}') {
            push_needle(&mut needles, word);
        }
    }

    needles.truncate(MAX_NEEDLES);
    needles
}

fn push_needle(needles: &mut Vec<String>, needle: &str) {
    let normalized = normalize_needle(needle);
    if !is_searchable_needle(&normalized) {
        return;
    }
    if !needles.iter().any(|item| item == &normalized) {
        needles.push(normalized);
    }
}

fn is_searchable_needle(needle: &str) -> bool {
    let char_count = needle.chars().count();
    char_count >= 3
        || needle
            .chars()
            .any(|ch| !ch.is_ascii() && !ch.is_whitespace())
}

fn normalize_needle(needle: &str) -> String {
    needle
        .trim()
        .trim_matches(|ch: char| ch.is_ascii_punctuation() && ch != '\\' && ch != '@')
        .to_string()
}

fn compact_snippet(input: &str) -> String {
    let snippet = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if snippet.chars().count() > 120 {
        snippet.chars().take(117).collect::<String>() + "..."
    } else {
        snippet
    }
}

fn build_markdown_diagnostic(
    source_file: &Path,
    line: usize,
    column: usize,
    diagnostic: &str,
    snippet: &str,
) -> MarkdownDiagnostic {
    let line = line.max(1);
    let column = column.max(1);
    let context = read_context(source_file, line, column, snippet);
    MarkdownDiagnostic {
        file: source_file.display().to_string(),
        line,
        column,
        kind: classify_diagnostic(diagnostic).to_string(),
        snippet: compact_snippet(snippet),
        message: compact_snippet(first_relevant_message(diagnostic)),
        context,
        help: diagnostic_help(diagnostic).map(str::to_string),
    }
}

fn read_context(
    source_file: &Path,
    line: usize,
    column: usize,
    fallback: &str,
) -> Vec<DiagnosticContextLine> {
    let Ok(content) = std::fs::read_to_string(source_file) else {
        let display = source_context_line(fallback, column, None);
        return vec![DiagnosticContextLine {
            line,
            text: display.text,
            primary: true,
            marker_column: Some(display.marker_column),
        }];
    };

    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }
    if line > lines.len() {
        let display = source_context_line(fallback, column, None);
        return vec![DiagnosticContextLine {
            line,
            text: display.text,
            primary: true,
            marker_column: Some(display.marker_column),
        }];
    }

    let focus_line = lines
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or(fallback);
    let focus_display = source_context_line(focus_line, column, None);
    let start = line.saturating_sub(CONTEXT_RADIUS).max(1);
    let end = (line + CONTEXT_RADIUS).min(lines.len());
    (start..=end)
        .filter_map(|line_no| {
            lines.get(line_no.saturating_sub(1)).map(|text| {
                let display = if line_no == line {
                    focus_display.clone()
                } else {
                    source_context_line(text, column, Some(focus_display.start_column))
                };
                DiagnosticContextLine {
                    line: line_no,
                    text: display.text,
                    primary: line_no == line,
                    marker_column: (line_no == line).then_some(display.marker_column),
                }
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SourceContextDisplay {
    text: String,
    start_column: usize,
    marker_column: usize,
}

fn source_context_line(
    line: &str,
    focus_column: usize,
    preferred_start: Option<usize>,
) -> SourceContextDisplay {
    let line_len = line.chars().count();
    let focus_column = focus_column.max(1).min(line_len.saturating_add(1));
    if line_len <= MAX_CONTEXT_CHARS {
        return SourceContextDisplay {
            text: line.to_string(),
            start_column: 1,
            marker_column: focus_column.min(line_len.saturating_add(1)).max(1),
        };
    }

    let max_body_chars = MAX_CONTEXT_CHARS.saturating_sub(6).max(20);
    let max_start = line_len.saturating_sub(max_body_chars).saturating_add(1);
    let centered_start = focus_column
        .saturating_sub(max_body_chars / 2)
        .max(1)
        .min(max_start);
    let start_column = preferred_start
        .unwrap_or(centered_start)
        .max(1)
        .min(max_start);
    let has_left = start_column > 1;
    let prefix = if has_left { "..." } else { "" };
    let available = MAX_CONTEXT_CHARS.saturating_sub(prefix.chars().count());
    let remaining_chars = line_len.saturating_sub(start_column).saturating_add(1);
    let has_right = remaining_chars > available;
    let take_chars = if has_right {
        available.saturating_sub(3)
    } else {
        available
    };
    let body = line
        .chars()
        .skip(start_column.saturating_sub(1))
        .take(take_chars)
        .collect::<String>();
    let suffix = if has_right { "..." } else { "" };
    let marker_column = if focus_column < start_column {
        prefix.chars().count() + 1
    } else {
        prefix.chars().count() + focus_column.saturating_sub(start_column) + 1
    };

    SourceContextDisplay {
        text: format!("{}{}{}", prefix, body, suffix),
        start_column,
        marker_column: marker_column.max(1),
    }
}

fn project_root(entry_file: &Path) -> PathBuf {
    let start = if entry_file.is_dir() {
        entry_file
    } else {
        entry_file.parent().unwrap_or_else(|| Path::new("."))
    };
    let mut current = start.to_path_buf();
    loop {
        if current.join(".omnidoc.toml").exists() || current.join(".git").exists() {
            return current;
        }
        if !current.pop() {
            break;
        }
    }
    start.to_path_buf()
}

fn markdown_candidates(entry_file: &Path) -> Vec<PathBuf> {
    let root = project_root(entry_file);
    let mut candidates = Vec::new();
    if entry_file.exists() {
        candidates.push(entry_file.to_path_buf());
    }
    let mut discovered = WalkDir::new(&root)
        .into_iter()
        .filter_entry(|entry| should_descend(entry.path(), &root))
        .flatten()
        .filter(|entry| entry.file_type().is_file() && is_markdown_file(entry.path()))
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    discovered.sort();
    for path in discovered {
        if !candidates.iter().any(|candidate| candidate == &path) {
            candidates.push(path);
        }
    }
    candidates
}

fn should_descend(path: &Path, root: &Path) -> bool {
    if path == root {
        return true;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    !matches!(
        name,
        ".git" | "build" | "target" | ".target" | ".cache" | ".omnidoc-cache" | "node_modules"
    )
}

fn is_markdown_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "markdown")
    )
}

fn classify_diagnostic(diagnostic: &str) -> &'static str {
    let lower = diagnostic.to_ascii_lowercase();
    if lower.contains("undefined control sequence") {
        "undefined_control_sequence"
    } else if lower.contains("missing $ inserted") {
        "missing_math_delimiter"
    } else if lower.contains("misplaced alignment tab character") {
        "misplaced_alignment_tab"
    } else if lower.contains("unicode character") {
        "unicode_character"
    } else if lower.contains("yaml") && (lower.contains("line") || lower.contains("column")) {
        "yaml"
    } else if lower.contains("citation") || lower.contains("citeproc") {
        "citation"
    } else if lower.contains(".sty") && lower.contains("not found") {
        "missing_latex_package"
    } else if lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("does not exist")
    {
        "missing_file"
    } else if lower.contains("latex error") || diagnostic.trim_start().starts_with('!') {
        "latex_error"
    } else if lower.contains("pandoc:") {
        "pandoc_error"
    } else if lower.contains("table") || lower.contains("tabular") {
        "table"
    } else {
        "unknown"
    }
}

fn first_relevant_message(diagnostic: &str) -> &str {
    diagnostic
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with("Command exited with code")
                && !line.starts_with("Executing:")
                && (line.starts_with('!')
                    || line.starts_with("pandoc:")
                    || line.starts_with("l.")
                    || line.contains("Error")
                    || line.contains("error")
                    || line.contains("not found")
                    || line.contains("does not exist")
                    || line.contains("Warning:"))
        })
        .or_else(|| {
            diagnostic
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
        })
        .unwrap_or("No diagnostic message available")
}

fn diagnostic_help(diagnostic: &str) -> Option<&'static str> {
    let lower = diagnostic.to_ascii_lowercase();
    if lower.contains("undefined control sequence") {
        Some("Check raw LaTeX commands, math macros, and required packages near this Markdown location.")
    } else if lower.contains("missing $ inserted") {
        Some("Check unmatched math delimiters, underscores, carets, or raw LaTeX near this line.")
    } else if lower.contains("misplaced alignment tab character") {
        Some("Escape literal ampersands as \\& outside tables and LaTeX alignment environments.")
    } else if lower.contains("unicode character") {
        Some("Check this character against the selected LaTeX engine, font, and template packages.")
    } else if lower.contains(".sty") && lower.contains("not found") {
        Some("Install the missing LaTeX package or remove the package use from template/header-includes.")
    } else if lower.contains("not found")
        || lower.contains("does not exist")
        || lower.contains("no such file")
    {
        Some("Check the referenced path and Pandoc resource_path; paths are resolved relative to the project and input file.")
    } else if lower.contains("citation") || lower.contains("citeproc") {
        Some("Check bibliography files, citation keys, and CSL/citeproc configuration.")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_markdown_diagnostic, classify_diagnostic, extract_needles, find_column,
        locate_direct_markdown_location, locate_in_raw_markdown, markdown_candidates,
        parse_data_pos_start, path_suffix_matches, source_context_line,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn extracts_latex_macro_from_diagnostic() {
        let needles = extract_needles("! Undefined control sequence.\nl.42 \\badmacro");

        assert!(needles.iter().any(|needle| needle == "\\badmacro"));
    }

    #[test]
    fn extracts_resource_paths_from_pandoc_diagnostic() {
        let needles = extract_needles("pandoc: images/missing.png: openBinaryFile: does not exist");

        assert!(needles.iter().any(|needle| needle == "images/missing.png"));
    }

    #[test]
    fn extracts_common_latex_and_citation_needles() {
        let needles = extract_needles(
            "Package inputenc Error: Unicode character ² (U+00B2)\n\
             LaTeX Error: File `custompkg.sty' not found.\n\
             [WARNING] Citeproc: citation @doe2024 not found\n\
             l.42 value with \\badmacro",
        );

        assert!(needles.iter().any(|needle| needle == "²"));
        assert!(needles.iter().any(|needle| needle == "custompkg.sty"));
        assert!(needles.iter().any(|needle| needle == "custompkg"));
        assert!(needles.iter().any(|needle| needle == "@doe2024"));
        assert!(needles.iter().any(|needle| needle == "\\badmacro"));
    }

    #[test]
    fn parses_sourcepos_line_and_column() {
        assert_eq!(parse_data_pos_start("12:5-12:20"), Some((12, 5)));
        assert_eq!(parse_data_pos_start("9"), Some((9, 1)));
    }

    #[test]
    fn finds_one_based_column() {
        assert_eq!(find_column("alpha \\badmacro beta", "\\badmacro"), Some(7));
        assert_eq!(find_column("Alpha beta", "alpha"), Some(1));
    }

    #[test]
    fn renders_structured_markdown_diagnostic() {
        let diagnostic = build_markdown_diagnostic(
            Path::new("main.md"),
            4,
            3,
            "! Undefined control sequence.",
            "  $\\badmacro$",
        );

        assert_eq!(diagnostic.kind, "undefined_control_sequence");
        assert!(diagnostic.render().contains("main.md:4:3"));
        assert!(diagnostic.render().contains("$\\badmacro$"));
    }

    #[test]
    fn classifies_common_diagnostics() {
        assert_eq!(
            classify_diagnostic("pandoc: image.png: openBinaryFile: does not exist"),
            "missing_file"
        );
        assert_eq!(classify_diagnostic("LaTeX Error: Bad table"), "latex_error");
        assert_eq!(
            classify_diagnostic("! Missing $ inserted."),
            "missing_math_delimiter"
        );
        assert_eq!(
            classify_diagnostic("Package inputenc Error: Unicode character ² (U+00B2)"),
            "unicode_character"
        );
    }

    #[test]
    fn recognizes_direct_file_line_column_diagnostics() {
        let root = std::env::temp_dir().join(format!(
            "omnidoc-source-map-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("root");
        let entry = root.join("main.md");
        fs::write(&entry, "# Title\n\n![x](missing.png)\n").expect("entry");

        let diagnostic =
            locate_direct_markdown_location(&entry, "pandoc: main.md:3:5: resource missing")
                .expect("diagnostic");

        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 5);
        assert!(diagnostic.snippet.contains("missing.png"));
        assert!(path_suffix_matches(&entry, "main.md"));
        assert!(diagnostic.render().contains("3 | ![x](missing.png)"));
        assert!(diagnostic.render().contains("|     ^"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recognizes_entry_line_column_diagnostics_without_file() {
        let root = std::env::temp_dir().join(format!(
            "omnidoc-source-map-line-column-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("root");
        let entry = root.join("main.md");
        fs::write(&entry, "---\ntitle: [broken\n---\n").expect("entry");

        let diagnostic = locate_direct_markdown_location(
            &entry,
            "YAML parse exception at line 2, column 8: did not find expected node content",
        )
        .expect("diagnostic");

        assert_eq!(diagnostic.line, 2);
        assert_eq!(diagnostic.column, 8);
        assert_eq!(diagnostic.kind, "yaml");
        assert!(diagnostic.render().contains("2 | title: [broken"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn searches_project_markdown_candidates() {
        let root = std::env::temp_dir().join(format!(
            "omnidoc-source-map-candidates-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("chapters")).expect("chapters");
        fs::create_dir_all(root.join("build")).expect("build");
        let entry = root.join("main.md");
        let chapter = root.join("chapters").join("intro.md");
        fs::write(&entry, "# Main\n\ninclude=\"chapters/intro.md\"\n").expect("entry");
        fs::write(&chapter, "# Intro\n\n$\\badmacro$\n").expect("chapter");
        fs::write(root.join("build").join("ignored.md"), "$\\badmacro$\n").expect("ignored");

        let candidates = markdown_candidates(&entry);
        assert!(candidates.iter().any(|path| path == &chapter));
        assert!(!candidates
            .iter()
            .any(|path| path.ends_with("build/ignored.md")));

        let diagnostic = locate_in_raw_markdown(
            &entry,
            "! Undefined control sequence.\nl.42 \\badmacro",
            &["\\badmacro".to_string()],
        )
        .expect("diagnostic");

        assert!(diagnostic.file.ends_with("chapters/intro.md"));
        assert_eq!(diagnostic.line, 3);
        assert_eq!(diagnostic.column, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn searches_from_project_root_when_entry_is_in_subdirectory() {
        let root = std::env::temp_dir().join(format!(
            "omnidoc-source-map-root-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).expect("src");
        fs::create_dir_all(root.join("chapters")).expect("chapters");
        fs::write(
            root.join(".omnidoc.toml"),
            "[project]\nentry = \"src/main.md\"\n",
        )
        .expect("config");
        let entry = root.join("src").join("main.md");
        let chapter = root.join("chapters").join("intro.md");
        fs::write(&entry, "# Main\n").expect("entry");
        fs::write(&chapter, "Text with \\badmacro here\n").expect("chapter");

        let candidates = markdown_candidates(&entry);

        assert!(candidates.iter().any(|path| path == &chapter));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn crops_long_context_lines_around_marker() {
        let line = format!("{}\\badmacro{}", "a".repeat(180), "b".repeat(80));
        let marker_column = find_column(&line, "\\badmacro").expect("column");

        let display = source_context_line(&line, marker_column, None);

        assert!(display.text.starts_with("..."));
        assert!(display.text.contains("\\badmacro"));
        assert!(display.text.chars().count() <= super::MAX_CONTEXT_CHARS);
        assert!(display.marker_column < display.text.chars().count());
    }
}
