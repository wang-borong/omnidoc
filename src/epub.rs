use crate::error::{OmniDocError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};
use zip::{CompressionMethod, ZipArchive};

const MAX_INSPECTED_ENTRY_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EpubCompatibilityReport {
    pub profile: String,
    pub profile_version: u32,
    pub valid: bool,
    pub reader_matrix: Vec<String>,
    pub checks: Vec<EpubCompatibilityCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EpubCompatibilityCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

pub fn is_supported_epub_profile(profile: &str) -> bool {
    profile == "readium"
}

pub fn validate_epub(path: &Path, profile: &str) -> Result<EpubCompatibilityReport> {
    if !is_supported_epub_profile(profile) {
        return Err(OmniDocError::Config(format!(
            "Unsupported EPUB compatibility profile '{}'. Supported profiles: readium",
            profile
        )));
    }
    validate_readium(path)
}

fn validate_readium(path: &Path) -> Result<EpubCompatibilityReport> {
    let file = File::open(path).map_err(OmniDocError::Io)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| OmniDocError::Other(format!("Invalid EPUB ZIP: {}", error)))?;
    let mut checks = Vec::new();
    let mut names = BTreeSet::new();
    let mut duplicate_names = Vec::new();
    let mut unsafe_names = Vec::new();
    let mut content = BTreeMap::<String, String>::new();
    let mut first_entry = None;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| OmniDocError::Other(format!("Cannot read EPUB entry: {}", error)))?;
        let name = entry.name().replace('\\', "/");
        if index == 0 {
            first_entry = Some((name.clone(), entry.compression()));
        }
        if !names.insert(name.clone()) {
            duplicate_names.push(name.clone());
        }
        if !safe_archive_path(&name) {
            unsafe_names.push(name.clone());
        }
        if entry.is_file() && entry.size() <= MAX_INSPECTED_ENTRY_BYTES && is_text_entry(&name) {
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).map_err(OmniDocError::Io)?;
            if let Ok(text) = String::from_utf8(bytes) {
                content.insert(name, text);
            }
        }
    }

    let mimetype = content.get("mimetype").map(|value| value.trim());
    push_check(
        &mut checks,
        "epub-mimetype",
        first_entry.as_ref().is_some_and(|(name, compression)| {
            name == "mimetype" && *compression == CompressionMethod::Stored
        }) && mimetype == Some("application/epub+zip"),
        "mimetype is the first uncompressed entry and declares application/epub+zip",
    );
    push_check(
        &mut checks,
        "safe-entry-paths",
        unsafe_names.is_empty(),
        if unsafe_names.is_empty() {
            "all archive paths are relative and normalized".to_string()
        } else {
            format!("unsafe archive paths: {}", unsafe_names.join(", "))
        },
    );
    push_check(
        &mut checks,
        "unique-entry-paths",
        duplicate_names.is_empty(),
        if duplicate_names.is_empty() {
            "archive entry names are unique".to_string()
        } else {
            format!("duplicate archive entries: {}", duplicate_names.join(", "))
        },
    );
    push_check(
        &mut checks,
        "container",
        names.contains("META-INF/container.xml"),
        "META-INF/container.xml is present",
    );

    let opf_entries = names
        .iter()
        .filter(|name| name.ends_with(".opf"))
        .cloned()
        .collect::<Vec<_>>();
    let epub3 = opf_entries.iter().any(|name| {
        content
            .get(name)
            .is_some_and(|text| text.contains("version=\"3.0\"") || text.contains("version='3.0'"))
    });
    push_check(
        &mut checks,
        "epub3-package",
        !opf_entries.is_empty() && epub3,
        "an EPUB 3 package document is present",
    );
    push_check(
        &mut checks,
        "navigation-document",
        names.iter().any(|name| name.ends_with("nav.xhtml")),
        "an EPUB navigation document is present",
    );
    push_check(
        &mut checks,
        "stylesheets",
        names.iter().any(|name| name.ends_with(".css")),
        "at least one packaged stylesheet is present",
    );
    push_check(
        &mut checks,
        "reading-content",
        names
            .iter()
            .any(|name| name.ends_with(".xhtml") || name.ends_with(".html")),
        "at least one HTML/XHTML reading document is present",
    );

    let xhtml = content
        .iter()
        .filter(|(name, _)| name.ends_with(".xhtml") || name.ends_with(".html"))
        .map(|(_, text)| text.as_str())
        .collect::<Vec<_>>();
    let has_mathml = xhtml.iter().any(|text| text.contains("<math"));
    let mathml_namespaced = !has_mathml
        || xhtml.iter().all(|text| {
            !text.contains("<math") || text.contains("xmlns=\"http://www.w3.org/1998/Math/MathML\"")
        });
    push_check(
        &mut checks,
        "mathml-namespace",
        mathml_namespaced,
        "MathML uses the EPUB-compatible MathML namespace",
    );

    let annotation =
        Regex::new(r#"(?s)<annotation\b[^>]*>(.*?)</annotation>"#).expect("annotation regex");
    let mut leaked_annotations = Vec::new();
    for text in &xhtml {
        let visible = annotation.replace_all(text, "");
        for capture in annotation.captures_iter(text) {
            let source = capture
                .get(1)
                .map(|value| value.as_str().trim())
                .unwrap_or("");
            if source.len() >= 4 && visible.contains(source) {
                leaked_annotations.push(source.chars().take(80).collect::<String>());
            }
        }
    }
    push_check(
        &mut checks,
        "mathml-annotation-visibility",
        leaked_annotations.is_empty(),
        if leaked_annotations.is_empty() {
            "MathML source annotations are not duplicated as visible text".to_string()
        } else {
            format!(
                "visible MathML annotations: {}",
                leaked_annotations.join(", ")
            )
        },
    );

    let missing_references = missing_local_references(&content, &names);
    push_check(
        &mut checks,
        "packaged-resources",
        missing_references.is_empty(),
        if missing_references.is_empty() {
            "local HTML resources resolve to packaged archive entries".to_string()
        } else {
            format!(
                "missing packaged resources: {}",
                missing_references.join(", ")
            )
        },
    );

    let valid = checks.iter().all(|check| check.passed);
    Ok(EpubCompatibilityReport {
        profile: "readium".to_string(),
        profile_version: 1,
        valid,
        reader_matrix: vec![
            "Readium/Thorium".to_string(),
            "Calibre".to_string(),
            "Apple Books".to_string(),
        ],
        checks,
    })
}

fn push_check(
    checks: &mut Vec<EpubCompatibilityCheck>,
    name: &str,
    passed: bool,
    detail: impl Into<String>,
) {
    checks.push(EpubCompatibilityCheck {
        name: name.to_string(),
        passed,
        detail: detail.into(),
    });
}

fn safe_archive_path(name: &str) -> bool {
    let path = Path::new(name);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_text_entry(name: &str) -> bool {
    name == "mimetype"
        || [".xml", ".opf", ".ncx", ".xhtml", ".html", ".css"]
            .iter()
            .any(|extension| name.ends_with(extension))
}

fn missing_local_references(
    content: &BTreeMap<String, String>,
    names: &BTreeSet<String>,
) -> Vec<String> {
    let reference = Regex::new(r#"(?:href|src)=["']([^"']+)["']"#).expect("reference regex");
    let root_reference = Regex::new(r#"full-path=["']([^"']+)["']"#).expect("root reference regex");
    let css_reference =
        Regex::new(r#"url\(\s*["']?([^\)"']+)["']?\s*\)"#).expect("CSS reference regex");
    let mut missing = BTreeSet::new();
    for (document, text) in content
        .iter()
        .filter(|(name, _)| name.as_str() != "mimetype")
    {
        let base = Path::new(document)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        for capture in reference.captures_iter(text) {
            let target = capture.get(1).map(|value| value.as_str()).unwrap_or("");
            check_local_reference(document, base, target, names, &mut missing);
        }
        for capture in root_reference.captures_iter(text) {
            let target = capture.get(1).map(|value| value.as_str()).unwrap_or("");
            check_local_reference(document, Path::new(""), target, names, &mut missing);
        }
        if document.ends_with(".css") {
            for capture in css_reference.captures_iter(text) {
                let target = capture.get(1).map(|value| value.as_str()).unwrap_or("");
                check_local_reference(document, base, target, names, &mut missing);
            }
        }
    }
    missing.into_iter().collect()
}

fn check_local_reference(
    document: &str,
    base: &Path,
    target: &str,
    names: &BTreeSet<String>,
    missing: &mut BTreeSet<String>,
) {
    let target = target.split(['#', '?']).next().unwrap_or("").trim();
    if target.is_empty()
        || target.starts_with("data:")
        || target.starts_with("http:")
        || target.starts_with("https:")
        || target.starts_with("mailto:")
    {
        return;
    }
    let Some(resolved) = normalize_archive_reference(base, target) else {
        missing.insert(format!("{} -> {}", document, target));
        return;
    };
    if !names.contains(&resolved) {
        missing.insert(format!("{} -> {}", document, target));
    }
}

fn normalize_archive_reference(base: &Path, target: &str) -> Option<String> {
    let mut parts = Vec::<String>::new();
    for component in base.join(target).components() {
        match component {
            Component::Normal(value) => parts.push(value.to_string_lossy().to_string()),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::{normalize_archive_reference, safe_archive_path, validate_epub};
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    #[test]
    fn normalizes_safe_epub_references() {
        assert_eq!(
            normalize_archive_reference(Path::new("EPUB/text"), "../media/cover.svg"),
            Some("EPUB/media/cover.svg".to_string())
        );
        assert!(normalize_archive_reference(Path::new("EPUB"), "../../outside").is_none());
        assert!(safe_archive_path("EPUB/text/chapter.xhtml"));
        assert!(!safe_archive_path("../outside"));
    }

    #[test]
    fn validates_readium_epub_structure_and_resources() {
        let directory = tempfile::tempdir().expect("temporary EPUB");
        let epub = directory.path().join("book.epub");
        let file = File::create(&epub).expect("EPUB file");
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(
                "mimetype",
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("mimetype entry");
        writer.write_all(b"application/epub+zip").expect("mimetype");
        for (name, content) in [
            ("META-INF/container.xml", "<container/>"),
            ("EPUB/content.opf", "<package version=\"3.0\"/>"),
            ("EPUB/nav.xhtml", "<html><body><nav/></body></html>"),
            (
                "EPUB/styles/book.css",
                ".omni-display-math { text-align: center; }",
            ),
            (
                "EPUB/text/chapter.xhtml",
                r#"<html><head><link href="../styles/book.css"/></head><body><math xmlns="http://www.w3.org/1998/Math/MathML"><semantics><mi>x</mi><annotation encoding="application/x-tex">x^2</annotation></semantics></math></body></html>"#,
            ),
        ] {
            writer
                .start_file(name, SimpleFileOptions::default())
                .expect("EPUB entry");
            writer.write_all(content.as_bytes()).expect("EPUB content");
        }
        writer.finish().expect("finish EPUB");

        let report = validate_epub(&epub, "readium").expect("validate EPUB");
        assert!(report.valid, "{:#?}", report.checks);
        assert!(report
            .checks
            .iter()
            .any(|check| check.name == "packaged-resources" && check.passed));
    }
}
