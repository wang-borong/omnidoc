use crate::build::executor::BuildExecutor;
use crate::error::Result;
use regex::Regex;
use serde_json::Value;
use std::path::Path;

const MAX_NEEDLES: usize = 8;

#[derive(Debug, Clone)]
struct SourceSpan {
    line: usize,
    column: usize,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDiagnostic {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub kind: String,
    pub snippet: String,
    pub message: String,
}

impl MarkdownDiagnostic {
    pub fn render(&self) -> String {
        format!(
            "Markdown source diagnostic: {}:{}:{}: {}\n  {}\n  note: {}",
            self.file, self.line, self.column, self.kind, self.snippet, self.message
        )
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
        if needle.len() < 3 {
            continue;
        }

        if let Some(span) = spans.iter().find(|span| {
            span.text
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
        }) {
            return Some(build_markdown_diagnostic(
                entry_file,
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
    collect_spans(&value, &mut spans);
    Ok(spans)
}

fn collect_spans(value: &Value, spans: &mut Vec<SourceSpan>) {
    if let Some((line, column)) = data_pos_start(value) {
        let text = collect_text(value);
        if !text.trim().is_empty() {
            spans.push(SourceSpan { line, column, text });
        }
    }

    match value {
        Value::Array(items) => {
            for item in items {
                collect_spans(item, spans);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_spans(item, spans);
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
    let content = std::fs::read_to_string(entry_file).ok()?;
    for needle in needles {
        let needle = normalize_needle(needle);
        if needle.len() < 3 {
            continue;
        }

        for (index, line) in content.lines().enumerate() {
            if let Some(column) = find_column(line, &needle) {
                return Some(build_markdown_diagnostic(
                    entry_file,
                    index + 1,
                    column,
                    diagnostic,
                    line,
                ));
            }
        }
    }
    None
}

fn locate_direct_markdown_location(
    entry_file: &Path,
    diagnostic: &str,
) -> Option<MarkdownDiagnostic> {
    let location_re =
        Regex::new(r"(?m)(?P<file>[^\s:]+\.m(?:d|arkdown)):(?P<line>\d+):(?P<column>\d+)")
            .expect("location regex");
    let entry_content = std::fs::read_to_string(entry_file).ok()?;
    for capture in location_re.captures_iter(diagnostic) {
        let file = capture.name("file")?.as_str();
        let line = capture.name("line")?.as_str().parse::<usize>().ok()?;
        let column = capture
            .name("column")
            .and_then(|value| value.as_str().parse::<usize>().ok())
            .unwrap_or(1);
        if !diagnostic_path_matches_entry(file, entry_file) {
            continue;
        }
        let snippet = entry_content
            .lines()
            .nth(line.saturating_sub(1))
            .unwrap_or("");
        return Some(build_markdown_diagnostic(
            entry_file, line, column, diagnostic, snippet,
        ));
    }
    None
}

fn diagnostic_path_matches_entry(diagnostic_path: &str, entry_file: &Path) -> bool {
    let path = Path::new(diagnostic_path);
    if path.is_absolute() {
        return path == entry_file;
    }
    if path == entry_file {
        return true;
    }
    entry_file
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| diagnostic_path.ends_with(name))
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
    let resource_re =
        Regex::new(r#"(?i)(?:pandoc:\s*)?([^\s:'"`]+?\.(?:png|jpg|jpeg|svg|pdf|bib|csl|md|markdown|tex|csv|tsv|yaml|yml|json))"#)
            .expect("resource regex");

    for capture in macro_re.find_iter(diagnostic) {
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

    for line in diagnostic
        .lines()
        .filter(|line| line.trim_start().starts_with("l."))
    {
        for word in line.split(|ch: char| ch.is_whitespace() || ch == '{' || ch == '}') {
            push_needle(&mut needles, word);
        }
    }

    needles.truncate(MAX_NEEDLES);
    needles
}

fn push_needle(needles: &mut Vec<String>, needle: &str) {
    let normalized = normalize_needle(needle);
    if normalized.len() < 3 {
        return;
    }
    if !needles.iter().any(|item| item == &normalized) {
        needles.push(normalized);
    }
}

fn normalize_needle(needle: &str) -> String {
    needle
        .trim()
        .trim_matches(|ch: char| ch.is_ascii_punctuation() && ch != '\\')
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
    entry_file: &Path,
    line: usize,
    column: usize,
    diagnostic: &str,
    snippet: &str,
) -> MarkdownDiagnostic {
    MarkdownDiagnostic {
        file: entry_file.display().to_string(),
        line,
        column,
        kind: classify_diagnostic(diagnostic).to_string(),
        snippet: compact_snippet(snippet),
        message: compact_snippet(first_relevant_message(diagnostic)),
    }
}

fn classify_diagnostic(diagnostic: &str) -> &'static str {
    let lower = diagnostic.to_ascii_lowercase();
    if lower.contains("undefined control sequence") {
        "undefined_control_sequence"
    } else if lower.contains("not found")
        || lower.contains("no such file")
        || lower.contains("does not exist")
    {
        "missing_file"
    } else if lower.contains("latex error") || diagnostic.trim_start().starts_with('!') {
        "latex_error"
    } else if lower.contains("pandoc:") {
        "pandoc_error"
    } else if lower.contains("citation") || lower.contains("citeproc") {
        "citation"
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
        .find(|line| !line.is_empty())
        .unwrap_or("No diagnostic message available")
}

#[cfg(test)]
mod tests {
    use super::{
        build_markdown_diagnostic, classify_diagnostic, diagnostic_path_matches_entry,
        extract_needles, find_column, locate_direct_markdown_location, parse_data_pos_start,
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
        assert!(diagnostic_path_matches_entry("main.md", &entry));
        let _ = fs::remove_dir_all(root);
    }
}
