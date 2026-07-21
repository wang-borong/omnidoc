//! Consistent, hierarchy-first terminal diagnostics.

use crate::error::OmniDocError;
use console::{colors_enabled_stderr, style};
use std::io::{self, Write};

#[derive(Clone, Copy)]
enum Level {
    Error,
    Warning,
    Info,
}

impl Level {
    fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// Print a fatal OmniDoc error to stderr.
pub fn print_error(error: &OmniDocError) {
    let _ = write_error(&mut io::stderr().lock(), error, colors_enabled_stderr());
}

/// Print a recoverable warning to stderr.
pub fn warning(message: impl AsRef<str>) {
    let _ = write_diagnostic(
        &mut io::stderr().lock(),
        Level::Warning,
        None,
        message.as_ref(),
        None,
        colors_enabled_stderr(),
    );
}

/// Print a quiet informational diagnostic to stderr.
pub fn info(message: impl AsRef<str>) {
    let _ = write_diagnostic(
        &mut io::stderr().lock(),
        Level::Info,
        None,
        message.as_ref(),
        None,
        colors_enabled_stderr(),
    );
}

fn write_error(writer: &mut impl Write, error: &OmniDocError, color: bool) -> io::Result<()> {
    write_diagnostic(
        writer,
        Level::Error,
        Some(error.category()),
        &error.message(),
        error.help(),
        color,
    )
}

fn write_diagnostic(
    writer: &mut impl Write,
    level: Level,
    category: Option<&str>,
    message: &str,
    help: Option<&str>,
    color: bool,
) -> io::Result<()> {
    let mut lines = message.lines();
    let title = lines.next().unwrap_or("Unknown error").trim();
    let label = if color {
        match level {
            Level::Error => style(level.label())
                .red()
                .bold()
                .force_styling(true)
                .to_string(),
            Level::Warning => style(level.label())
                .yellow()
                .bold()
                .force_styling(true)
                .to_string(),
            Level::Info => style(level.label())
                .cyan()
                .bold()
                .force_styling(true)
                .to_string(),
        }
    } else {
        level.label().to_string()
    };

    writeln!(writer, "{label}: {title}")?;
    if let Some(category) = category {
        let context_label = if color {
            style("context").dim().force_styling(true).to_string()
        } else {
            "context".to_string()
        };
        writeln!(writer, "  {context_label}: {category}")?;
    }

    let details = lines.collect::<Vec<_>>();
    if details.iter().any(|line| !line.trim().is_empty()) {
        writeln!(writer)?;
        for line in details {
            if line.trim().is_empty() {
                writeln!(writer)?;
            } else {
                writeln!(writer, "  {}", line.trim_end())?;
            }
        }
    }

    if let Some(help) = help {
        let help_label = if color {
            style("help").cyan().bold().force_styling(true).to_string()
        } else {
            "help".to_string()
        };
        writeln!(writer, "  {help_label}: {help}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_error_has_a_clean_title_context_and_indented_details() {
        let error = OmniDocError::Project(
            "EPUB compatibility validation failed for profile 'readium'\n\
             - epub-mimetype: mimetype must be the first uncompressed entry\n\
             - navigation-document: navigation document is missing"
                .to_string(),
        );
        let mut output = Vec::new();

        write_error(&mut output, &error, false).expect("render error");

        assert_eq!(
            String::from_utf8(output).expect("UTF-8"),
            "error: EPUB compatibility validation failed for profile 'readium'\n\
             \x20\x20context: project\n\
             \n\
             \x20\x20- epub-mimetype: mimetype must be the first uncompressed entry\n\
             \x20\x20- navigation-document: navigation document is missing\n"
        );
    }

    #[test]
    fn contextual_help_is_rendered_last() {
        let error = OmniDocError::NotOmniDocProject("No .omnidoc.toml was found".to_string());
        let mut output = Vec::new();

        write_error(&mut output, &error, false).expect("render error");
        let output = String::from_utf8(output).expect("UTF-8");

        assert!(output.starts_with(
            "error: This is not an OmniDoc project: No .omnidoc.toml was found\n  context: project\n"
        ));
        assert!(output.contains("  help: Run this command inside an OmniDoc project"));
        assert!(!output.contains('✖'));
    }
}
