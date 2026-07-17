use crate::utils::fs;
use std::collections::HashSet;
use std::path::Path;

const MAX_SUMMARY_LINES: usize = 18;

/// Build a compact diagnostic block from command output.
pub fn summarize_command_output(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let mut combined = String::new();
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);

    if !stderr.trim().is_empty() {
        combined.push_str(&stderr);
        combined.push('\n');
    }
    if !stdout.trim().is_empty() {
        combined.push_str(&stdout);
    }

    summarize_text(&combined)
}

/// Read and summarize a LaTeX log file if it exists.
pub fn summarize_latex_log(log_path: &Path) -> Option<String> {
    if !fs::exists(log_path) {
        return None;
    }

    fs::read_to_string(log_path)
        .ok()
        .and_then(|content| summarize_text(&content))
}

fn summarize_text(content: &str) -> Option<String> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    let lines: Vec<&str> = content.lines().collect();

    for (index, raw_line) in lines.iter().enumerate() {
        let line = raw_line.trim();
        if !is_interesting_line(line) {
            continue;
        }

        push_unique(&mut selected, &mut seen, line);

        if is_latex_error_start(line) {
            for context_line in lines.iter().skip(index + 1).take(4) {
                let context = context_line.trim();
                if context.is_empty() {
                    continue;
                }
                push_unique(&mut selected, &mut seen, context);
                if selected.len() >= MAX_SUMMARY_LINES {
                    break;
                }
            }
        }

        if selected.len() >= MAX_SUMMARY_LINES {
            break;
        }
    }

    if selected.is_empty() {
        let fallback = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .rev()
            .take(8)
            .collect::<Vec<_>>();

        if fallback.is_empty() {
            return None;
        }

        selected = fallback.into_iter().rev().map(str::to_string).collect();
    }

    Some(selected.join("\n"))
}

fn push_unique(selected: &mut Vec<String>, seen: &mut HashSet<String>, line: &str) {
    if selected.len() >= MAX_SUMMARY_LINES {
        return;
    }

    let normalized = line.to_string();
    if seen.insert(normalized.clone()) {
        selected.push(normalized);
    }
}

fn is_latex_error_start(line: &str) -> bool {
    line.starts_with('!') || line.contains("LaTeX Error:")
}

fn is_interesting_line(line: &str) -> bool {
    if line.is_empty() {
        return false;
    }

    line.starts_with('!')
        || line.starts_with("l.")
        || line.starts_with("pandoc:")
        || line.contains("Error producing PDF")
        || line.contains("Undefined control sequence")
        || line.contains("Missing ")
        || line.contains("Emergency stop")
        || line.contains("Fatal error")
        || line.contains("LaTeX Error:")
        || (line.contains("Package ") && (line.contains(" Warning") || line.contains(" Error")))
        || line.contains("File ")
        || line.contains("not found")
        || line.contains("Overfull \\hbox")
        || line.contains("Underfull \\hbox")
        || line.contains("Warning:")
}

#[cfg(test)]
mod tests {
    use super::{summarize_command_output, summarize_text};

    #[test]
    fn summarizes_latex_errors_with_context() {
        let stderr = b"noise\n! Undefined control sequence.\nl.42 \\badmacro\nmore context\n";
        let summary = summarize_command_output(&[], stderr).expect("summary");

        assert!(summary.contains("Undefined control sequence"));
        assert!(summary.contains("l.42"));
        assert!(!summary.contains("noise"));
    }

    #[test]
    fn falls_back_to_tail_when_no_known_pattern_matches() {
        let stderr = b"first\nsecond\nthird\n";
        let summary = summarize_command_output(&[], stderr).expect("summary");

        assert!(summary.contains("second"));
        assert!(summary.contains("third"));
    }

    #[test]
    fn ignores_package_info_before_a_latex_error() {
        let content = "Package fontspec Info: loading font.\n\
Package hyperref Info: bookmarks enabled.\n\
! LaTeX Error: File `missing.sty' not found.\n\
l.12 \\usepackage{missing}\n";
        let summary = summarize_text(content).expect("summary");

        assert!(!summary.contains("Package fontspec Info"));
        assert!(summary.contains("missing.sty"));
        assert!(summary.contains("l.12"));
    }
}
