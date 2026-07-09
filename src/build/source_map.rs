use crate::build::executor::BuildExecutor;
use crate::error::Result;
use regex::Regex;
use serde_json::Value;
use std::path::Path;

const MAX_NEEDLES: usize = 8;

#[derive(Debug, Clone)]
struct SourceSpan {
    line: usize,
    text: String,
}

pub fn locate_markdown_error(
    executor: &BuildExecutor,
    entry_file: &Path,
    diagnostic: &str,
) -> Option<String> {
    let needles = extract_needles(diagnostic);
    if needles.is_empty() {
        return None;
    }

    if let Some(line_hint) = locate_in_raw_markdown(entry_file, &needles) {
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
            return Some(format!(
                "Likely Markdown source near line {}: {}",
                span.line,
                compact_snippet(&span.text)
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
    if let Some(line) = data_pos_line(value) {
        let text = collect_text(value);
        if !text.trim().is_empty() {
            spans.push(SourceSpan { line, text });
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

fn data_pos_line(value: &Value) -> Option<usize> {
    let attrs = find_attr_keyvals(value)?;
    for keyval in attrs {
        let pair = keyval.as_array()?;
        let key = pair.get(0)?.as_str()?;
        let data_pos = pair.get(1)?.as_str()?;
        if key == "data-pos" {
            let line = data_pos.split(':').next()?;
            return line.parse::<usize>().ok();
        }
    }
    None
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

fn locate_in_raw_markdown(entry_file: &Path, needles: &[String]) -> Option<String> {
    let content = std::fs::read_to_string(entry_file).ok()?;
    for needle in needles {
        let needle = normalize_needle(needle);
        if needle.len() < 3 {
            continue;
        }

        for (index, line) in content.lines().enumerate() {
            if line
                .to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
            {
                return Some(format!(
                    "Likely Markdown source near line {}: {}",
                    index + 1,
                    compact_snippet(line)
                ));
            }
        }
    }
    None
}

fn extract_needles(diagnostic: &str) -> Vec<String> {
    let mut needles = Vec::new();
    let macro_re = Regex::new(r"\\[A-Za-z@]+").expect("macro regex");
    let quoted_re = Regex::new(r#"[`'"]([^`'"]{3,80})[`'"]"#).expect("quoted regex");

    for capture in macro_re.find_iter(diagnostic) {
        push_needle(&mut needles, capture.as_str());
    }
    for capture in quoted_re.captures_iter(diagnostic) {
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

#[cfg(test)]
mod tests {
    use super::extract_needles;

    #[test]
    fn extracts_latex_macro_from_diagnostic() {
        let needles = extract_needles("! Undefined control sequence.\nl.42 \\badmacro");

        assert!(needles.iter().any(|needle| needle == "\\badmacro"));
    }
}
