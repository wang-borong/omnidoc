use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PandocOutputKind {
    Pdf,
    Html,
    Epub,
    Docx,
    Latex,
}

impl PandocOutputKind {
    pub(crate) fn from_config(config: &MergedConfig) -> Result<Self> {
        let requested = config.to.as_deref().or(config.pandoc_to_format.as_deref());
        Self::from_requested(requested)
    }

    pub(crate) fn from_requested(requested: Option<&str>) -> Result<Self> {
        let requested = requested.unwrap_or("pdf").trim().to_ascii_lowercase();
        match requested.as_str() {
            "" | "pdf" => Ok(Self::Pdf),
            "html" | "html4" | "html5" => Ok(Self::Html),
            "epub" | "epub2" | "epub3" => Ok(Self::Epub),
            "docx" => Ok(Self::Docx),
            "latex" | "tex" => Ok(Self::Latex),
            _ => Err(OmniDocError::UnsupportedDocumentType(format!(
                "Unsupported build output format '{}'. Supported formats: pdf, html, epub, docx, latex",
                requested
            ))),
        }
    }

    pub(crate) fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Epub => "epub",
            Self::Docx => "docx",
            Self::Latex => "tex",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Html => "HTML",
            Self::Epub => "EPUB",
            Self::Docx => "DOCX",
            Self::Latex => "LaTeX",
        }
    }

    pub(crate) fn default_to_format(self) -> Option<&'static str> {
        match self {
            Self::Pdf => None,
            Self::Html => Some("html"),
            Self::Epub => Some("epub3"),
            Self::Docx => Some("docx"),
            Self::Latex => Some("latex"),
        }
    }

    pub(crate) fn uses_latex_defaults(self) -> bool {
        matches!(self, Self::Pdf | Self::Latex)
    }

    pub(crate) fn supports_embed_resources(self) -> bool {
        matches!(self, Self::Html)
    }

    pub(crate) fn supports_standalone(self) -> bool {
        !matches!(self, Self::Docx)
    }

    pub(crate) fn config_key(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Epub => "epub",
            Self::Docx => "docx",
            Self::Latex => "latex",
        }
    }

    pub(crate) fn default_filters(self) -> &'static [&'static str] {
        const LATEX_FILTERS: &[&str] = &[
            "include-files.lua",
            "include-code-files.lua",
            "diagram-generator.lua",
            "admonition.lua",
            "ltblr.lua",
            "latex-patch.lua",
            "fonts-and-alignment.lua",
        ];
        const PORTABLE_FILTERS: &[&str] = &[
            "include-code-files.lua",
            "include-files.lua",
            "diagram-generator.lua",
            "admonition.lua",
            "fonts-and-alignment.lua",
        ];
        if self.uses_latex_defaults() {
            LATEX_FILTERS
        } else {
            PORTABLE_FILTERS
        }
    }

    pub(crate) fn filters(self, config: &MergedConfig) -> Vec<&str> {
        if config.pandoc_lua_filters.is_empty() {
            self.default_filters().to_vec()
        } else {
            config
                .pandoc_lua_filters
                .iter()
                .map(String::as_str)
                .collect()
        }
    }

    pub(crate) fn append_configured_options(
        self,
        config: &MergedConfig,
        options: &mut Vec<String>,
    ) {
        options.extend(config.pandoc_options.clone());
        if let Some(format_options) = config.pandoc_format_options.get(self.config_key()) {
            options.extend(format_options.clone());
        }
    }

    pub(crate) fn has_explicit_html_math(self, config: &MergedConfig) -> bool {
        config
            .pandoc_options
            .iter()
            .chain(
                config
                    .pandoc_format_options
                    .get(self.config_key())
                    .into_iter()
                    .flatten(),
            )
            .any(|option| is_html_math_option(option))
    }
}

pub(crate) fn is_supported_format_key(key: &str) -> bool {
    matches!(key, "pdf" | "html" | "epub" | "docx" | "latex")
}

fn is_html_math_option(option: &str) -> bool {
    ["--mathml", "--mathjax", "--katex", "--webtex", "--gladtex"]
        .iter()
        .any(|flag| option == *flag || option.starts_with(&format!("{}=", flag)))
}

#[cfg(test)]
mod tests {
    use super::{is_supported_format_key, PandocOutputKind};
    use crate::config::MergedConfig;
    use std::collections::BTreeMap;

    #[test]
    fn normalizes_writer_aliases_to_policy_kinds() {
        assert_eq!(
            PandocOutputKind::from_requested(Some("html5")).expect("html"),
            PandocOutputKind::Html
        );
        assert_eq!(
            PandocOutputKind::from_requested(Some("epub3")).expect("epub"),
            PandocOutputKind::Epub
        );
        assert_eq!(
            PandocOutputKind::from_requested(Some("tex")).expect("latex"),
            PandocOutputKind::Latex
        );
    }

    #[test]
    fn shares_format_options_math_detection_and_filter_policy() {
        let config = MergedConfig {
            pandoc_options: vec!["--toc-depth=1".to_string()],
            pandoc_format_options: BTreeMap::from([(
                "epub".to_string(),
                vec!["--toc-depth=3".to_string(), "--mathjax".to_string()],
            )]),
            ..Default::default()
        };
        let mut options = Vec::new();
        PandocOutputKind::Epub.append_configured_options(&config, &mut options);

        assert_eq!(options, ["--toc-depth=1", "--toc-depth=3", "--mathjax"]);
        assert!(PandocOutputKind::Epub.has_explicit_html_math(&config));
        assert_eq!(
            PandocOutputKind::Pdf
                .default_filters()
                .iter()
                .filter(|filter| **filter == "ltblr.lua")
                .count(),
            1
        );
        assert!(is_supported_format_key("docx"));
        assert!(!is_supported_format_key("html5"));
    }
}
