use super::package::{
    digest_files, ensure_pandoc_compatible, normalized_hex_color, normalized_output,
    package_records, package_spec, safe_relative_path, tracked_package_files, PackageKind,
    PackageRecord, PackageScope, ResolvedPackageIdentity, ThemePackage, ThemePackageMetadata,
    ThemePackageRequirements, ThemePackageResources, ThemeTokens,
};
use crate::config::MergedConfig;
use crate::error::{OmniDocError, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_THEME_INHERITANCE_DEPTH: usize = 64;

#[derive(Debug, Clone, Default)]
pub struct ThemeResources {
    pub html_css: Vec<PathBuf>,
    pub epub_css: Vec<PathBuf>,
    pub latex_packages: Vec<PathBuf>,
    pub latex_headers: Vec<PathBuf>,
    pub html_template: Option<PathBuf>,
    pub epub_template: Option<PathBuf>,
    pub latex_template: Option<PathBuf>,
    pub docx_reference_doc: Option<PathBuf>,
    pub pptx_reference_doc: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeRequirements {
    pub fonts: Vec<String>,
    pub system_latex_packages: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeMetadata {
    pub defaults: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedTheme {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub recommended_for: Vec<String>,
    pub compatibility: Option<String>,
    pub outputs: Vec<String>,
    pub resources: ThemeResources,
    pub requirements: ThemeRequirements,
    pub metadata: ThemeMetadata,
    pub packages: Vec<ResolvedPackageIdentity>,
    outputs_explicit: bool,
    tokens: ThemeTokens,
}

impl ResolvedTheme {
    pub fn supports_output(&self, output: &str) -> bool {
        let output = normalized_output(output);
        self.outputs.iter().any(|candidate| candidate == &output)
    }
}

#[derive(Debug, Clone, Default)]
pub struct GeneratedThemeAssets {
    pub css: Option<PathBuf>,
    pub latex_header: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThemeCatalogEntry {
    pub manifest_path: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub recommended_for: Vec<String>,
    pub compatibility: Option<String>,
    pub compatible_omnidoc: Option<String>,
    pub compatible_pandoc: Option<String>,
    pub source: String,
    pub scope: PackageScope,
    pub digest: Option<String>,
    pub outputs: Vec<String>,
    pub resources: ThemeCatalogResources,
    pub requirements: ThemeRequirements,
    pub metadata: ThemeMetadata,
    pub has_tokens: bool,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ThemeCatalogResources {
    pub html_css: Vec<String>,
    pub epub_css: Vec<String>,
    pub latex_packages: Vec<String>,
    pub latex_headers: Vec<String>,
    pub html_template: Option<String>,
    pub epub_template: Option<String>,
    pub latex_template: Option<String>,
    pub docx_reference_doc: Option<String>,
    pub pptx_reference_doc: Option<String>,
}

#[derive(Debug, Clone)]
struct ThemeDescriptor {
    manifest_path: PathBuf,
    id: String,
    name: String,
    version: String,
    description: Option<String>,
    compatible_omnidoc: String,
    compatible_pandoc: Option<String>,
    theme: ThemePackage,
    root: PathBuf,
    scope: PackageScope,
    source: String,
    digest: String,
    tracked_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyThemeManifest {
    manifest_version: u32,
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    recommended_for: Vec<String>,
    compatible_omnidoc: String,
    #[serde(default)]
    compatibility: Option<String>,
    #[serde(default)]
    resources: LegacyThemeResources,
    #[serde(default)]
    requirements: ThemePackageRequirements,
    #[serde(default)]
    metadata: ThemePackageMetadata,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LegacyThemeResources {
    #[serde(default)]
    html_css: Vec<String>,
    #[serde(default)]
    epub_css: Vec<String>,
    #[serde(default)]
    latex_packages: Vec<String>,
    #[serde(default)]
    latex_headers: Vec<String>,
    // Theme-owned filters were part of manifest v1. They are deliberately
    // ignored: semantic transformations now belong to the core pipeline or a
    // separately installed Pandoc Lua plugin.
    #[serde(default, rename = "lua_filters")]
    _lua_filters: Vec<String>,
    #[serde(default)]
    templates: Vec<String>,
    #[serde(default)]
    html_template: Option<String>,
    #[serde(default)]
    epub_template: Option<String>,
    #[serde(default)]
    latex_template: Option<String>,
    #[serde(default)]
    docx_reference_doc: Option<String>,
    #[serde(default)]
    pptx_reference_doc: Option<String>,
}

pub fn theme_catalog(
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<ThemeCatalogEntry>> {
    let mut entries = Vec::new();
    for inspection in super::package::discover_packages(PackageKind::Theme, project_root, config)? {
        let manifest = inspection.manifest.as_ref();
        let theme = manifest.and_then(|manifest| manifest.theme.as_ref());
        entries.push(ThemeCatalogEntry {
            manifest_path: inspection.manifest_path,
            id: manifest
                .map(|manifest| manifest.id.clone())
                .unwrap_or_else(|| "invalid-theme".to_string()),
            name: manifest
                .and_then(|manifest| manifest.name.clone())
                .or_else(|| manifest.map(|manifest| manifest.id.clone()))
                .unwrap_or_else(|| "Invalid theme".to_string()),
            version: manifest
                .map(|manifest| manifest.version.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            description: manifest.and_then(|manifest| manifest.description.clone()),
            category: theme.and_then(|theme| theme.category.clone()),
            recommended_for: theme
                .map(|theme| theme.recommended_for.clone())
                .unwrap_or_default(),
            compatibility: theme.and_then(|theme| theme.compatibility.clone()),
            compatible_omnidoc: manifest.map(|manifest| manifest.compatible_omnidoc.clone()),
            compatible_pandoc: manifest.and_then(|manifest| manifest.compatible_pandoc.clone()),
            source: inspection.source,
            scope: inspection.scope,
            digest: inspection.digest,
            outputs: theme.map(theme_outputs).unwrap_or_default(),
            resources: theme
                .map(|theme| catalog_resources(&theme.resources))
                .unwrap_or_default(),
            requirements: theme
                .map(|theme| normalized_requirements(&theme.requirements))
                .unwrap_or_default(),
            metadata: theme
                .map(|theme| normalized_metadata(&theme.metadata))
                .unwrap_or_default(),
            has_tokens: theme.is_some_and(|theme| !theme.tokens.is_empty()),
            valid: inspection.valid,
            errors: inspection.errors,
        });
    }
    entries.extend(builtin_theme_entries(config));
    entries.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| {
                match (
                    Version::parse(&left.version),
                    Version::parse(&right.version),
                ) {
                    (Ok(left), Ok(right)) => left.cmp(&right),
                    _ => left.version.cmp(&right.version),
                }
            })
            .then_with(|| right.scope.cmp(&left.scope))
    });
    Ok(entries)
}

pub fn resolve_selected_theme(
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Option<ResolvedTheme>> {
    let Some(id) = config.theme_name.as_deref() else {
        return Ok(None);
    };
    let request = config
        .theme_version
        .as_deref()
        .map(|version| format!("{id}@{version}"))
        .unwrap_or_else(|| id.to_string());
    let spec = package_spec(&request).map_err(|error| {
        OmniDocError::Config(format!("invalid selected theme '{request}': {error}"))
    })?;
    let resolved = resolve_theme_requirement(project_root, config, &spec, &mut Vec::new())?;
    if let Some(requested) = config.theme_compatibility.as_deref() {
        if resolved.compatibility.as_deref() != Some(requested) {
            return Err(OmniDocError::Config(format!(
                "theme '{}' compatibility '{}' does not match requested '{}'",
                resolved.id,
                resolved.compatibility.as_deref().unwrap_or("default"),
                requested
            )));
        }
    }
    ensure_pandoc_compatible(&resolved.packages, config)?;
    Ok(Some(resolved))
}

pub fn resolve_theme_request(
    project_root: Option<&Path>,
    config: &MergedConfig,
    request: &str,
) -> Result<ResolvedTheme> {
    let spec = package_spec(request)?;
    let resolved = resolve_theme_requirement(project_root, config, &spec, &mut Vec::new())?;
    ensure_pandoc_compatible(&resolved.packages, config)?;
    Ok(resolved)
}

pub(crate) fn resolve_theme_manifest(
    project_root: Option<&Path>,
    config: &MergedConfig,
    manifest_path: &Path,
) -> Result<ResolvedTheme> {
    let descriptor = theme_descriptors(project_root, config)?
        .into_iter()
        .find(|descriptor| descriptor.manifest_path == manifest_path)
        .ok_or_else(|| {
            OmniDocError::Config(format!(
                "theme package at '{}' is invalid or no longer installed",
                manifest_path.display()
            ))
        })?;
    let resolved = resolve_theme_descriptor(project_root, config, descriptor, &mut Vec::new())?;
    ensure_pandoc_compatible(&resolved.packages, config)?;
    Ok(resolved)
}

fn resolve_theme_requirement(
    project_root: Option<&Path>,
    config: &MergedConfig,
    spec: &super::package::PackageSpec,
    stack: &mut Vec<String>,
) -> Result<ResolvedTheme> {
    if stack.len() >= MAX_THEME_INHERITANCE_DEPTH {
        return Err(OmniDocError::Config(format!(
            "theme inheritance exceeds the maximum depth of {} while resolving '{}'",
            MAX_THEME_INHERITANCE_DEPTH, spec.id
        )));
    }
    let descriptors = theme_descriptors(project_root, config)?;
    let descriptor = select_descriptor(descriptors, spec)?;
    resolve_theme_descriptor(project_root, config, descriptor, stack)
}

fn resolve_theme_descriptor(
    project_root: Option<&Path>,
    config: &MergedConfig,
    descriptor: ThemeDescriptor,
    stack: &mut Vec<String>,
) -> Result<ResolvedTheme> {
    let cycle_key = format!("{}@{}", descriptor.id, descriptor.version);
    if let Some(index) = stack.iter().position(|entry| entry == &cycle_key) {
        let mut cycle = stack[index..].to_vec();
        cycle.push(cycle_key);
        return Err(OmniDocError::Config(format!(
            "theme inheritance cycle: {}",
            cycle.join(" -> ")
        )));
    }
    stack.push(cycle_key);
    let child = descriptor_to_resolved(&descriptor)?;
    let result = if let Some(parent) = descriptor.theme.extends.as_deref() {
        let parent = package_spec(parent)?;
        let parent = resolve_theme_requirement(project_root, config, &parent, stack)?;
        merge_themes(parent, child)
    } else {
        child
    };
    stack.pop();
    validate_resolved_theme_outputs(&result)?;
    Ok(result)
}

fn select_descriptor(
    descriptors: Vec<ThemeDescriptor>,
    spec: &super::package::PackageSpec,
) -> Result<ThemeDescriptor> {
    let mut candidates = descriptors
        .into_iter()
        .filter(|descriptor| descriptor.id == spec.id)
        .filter_map(|descriptor| {
            let version = Version::parse(&descriptor.version).ok()?;
            spec.matches_version(&descriptor.version)
                .then_some((version, descriptor))
        })
        .collect::<Vec<_>>();
    let priority = candidates
        .iter()
        .map(|(_, descriptor)| descriptor.scope.priority())
        .max()
        .ok_or_else(|| {
            OmniDocError::Config(format!(
                "theme '{}' is not installed or has no version satisfying {}",
                spec.id,
                spec.raw_requirement.as_deref().unwrap_or("*")
            ))
        })?;
    candidates.retain(|(_, descriptor)| descriptor.scope.priority() == priority);
    candidates
        .into_iter()
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, descriptor)| descriptor)
        .ok_or_else(|| OmniDocError::Config(format!("theme '{}' could not be resolved", spec.id)))
}

fn theme_descriptors(
    project_root: Option<&Path>,
    config: &MergedConfig,
) -> Result<Vec<ThemeDescriptor>> {
    let mut descriptors = package_records(PackageKind::Theme, project_root, config)?
        .into_iter()
        .filter_map(package_theme_descriptor)
        .collect::<Result<Vec<_>>>()?;
    descriptors.extend(builtin_theme_descriptors(config)?);
    Ok(descriptors)
}

fn package_theme_descriptor(record: PackageRecord) -> Option<Result<ThemeDescriptor>> {
    let theme = record.manifest.theme.clone()?;
    let files = match tracked_package_files(&record.root) {
        Ok(files) => files,
        Err(error) => return Some(Err(error)),
    };
    Some(Ok(ThemeDescriptor {
        manifest_path: record.root.join(super::package::PACKAGE_MANIFEST_FILE),
        id: record.manifest.id.clone(),
        name: record
            .manifest
            .name
            .clone()
            .unwrap_or_else(|| record.manifest.id.clone()),
        version: record.manifest.version.clone(),
        description: record.manifest.description.clone(),
        compatible_omnidoc: record.manifest.compatible_omnidoc.clone(),
        compatible_pandoc: record.manifest.compatible_pandoc.clone(),
        theme,
        root: record.root,
        scope: record.scope,
        source: record.source,
        digest: record.digest,
        tracked_files: files,
    }))
}

fn descriptor_to_resolved(descriptor: &ThemeDescriptor) -> Result<ResolvedTheme> {
    let resources = &descriptor.theme.resources;
    let resolved = ThemeResources {
        html_css: resolve_resource_list(&descriptor.root, &resources.html_css),
        epub_css: resolve_resource_list(&descriptor.root, &resources.epub_css),
        latex_packages: resolve_resource_list(&descriptor.root, &resources.latex_packages),
        latex_headers: resolve_resource_list(&descriptor.root, &resources.latex_headers),
        html_template: resolve_optional_resource(&descriptor.root, &resources.html_template),
        epub_template: resolve_optional_resource(&descriptor.root, &resources.epub_template),
        latex_template: resolve_optional_resource(&descriptor.root, &resources.latex_template),
        docx_reference_doc: resolve_optional_resource(
            &descriptor.root,
            &resources.docx_reference_doc,
        ),
        pptx_reference_doc: resolve_optional_resource(
            &descriptor.root,
            &resources.pptx_reference_doc,
        ),
    };
    Ok(ResolvedTheme {
        id: descriptor.id.clone(),
        name: descriptor.name.clone(),
        version: descriptor.version.clone(),
        description: descriptor.description.clone(),
        category: descriptor.theme.category.clone(),
        recommended_for: descriptor.theme.recommended_for.clone(),
        compatibility: descriptor.theme.compatibility.clone(),
        outputs: theme_outputs(&descriptor.theme),
        resources: resolved,
        requirements: normalized_requirements(&descriptor.theme.requirements),
        metadata: normalized_metadata(&descriptor.theme.metadata),
        packages: vec![ResolvedPackageIdentity {
            kind: PackageKind::Theme,
            scope: descriptor.scope,
            id: descriptor.id.clone(),
            version: descriptor.version.clone(),
            source: descriptor.source.clone(),
            digest: descriptor.digest.clone(),
            root: descriptor.root.clone(),
            tracked_files: descriptor.tracked_files.clone(),
            compatible_pandoc: descriptor.compatible_pandoc.clone(),
        }],
        outputs_explicit: descriptor.theme.outputs.is_some(),
        tokens: descriptor.theme.tokens.clone(),
    })
}

fn merge_themes(parent: ResolvedTheme, child: ResolvedTheme) -> ResolvedTheme {
    let mut resources = parent.resources;
    append_unique_paths(&mut resources.html_css, child.resources.html_css);
    append_unique_paths(&mut resources.epub_css, child.resources.epub_css);
    append_unique_paths(
        &mut resources.latex_packages,
        child.resources.latex_packages,
    );
    append_unique_paths(&mut resources.latex_headers, child.resources.latex_headers);
    resources.html_template = child.resources.html_template.or(resources.html_template);
    resources.epub_template = child.resources.epub_template.or(resources.epub_template);
    resources.latex_template = child.resources.latex_template.or(resources.latex_template);
    resources.docx_reference_doc = child
        .resources
        .docx_reference_doc
        .or(resources.docx_reference_doc);
    resources.pptx_reference_doc = child
        .resources
        .pptx_reference_doc
        .or(resources.pptx_reference_doc);

    let mut fonts = parent.requirements.fonts;
    append_unique_strings(&mut fonts, child.requirements.fonts);
    let mut latex_packages = parent.requirements.system_latex_packages;
    append_unique_strings(
        &mut latex_packages,
        child.requirements.system_latex_packages,
    );
    let mut metadata = parent.metadata.defaults;
    metadata.extend(child.metadata.defaults);
    let outputs = if child.outputs_explicit {
        child.outputs.clone()
    } else {
        let mut outputs = parent.outputs;
        append_unique_strings(&mut outputs, child.outputs.clone());
        outputs
    };
    let mut packages = parent.packages;
    packages.extend(child.packages);

    ResolvedTheme {
        id: child.id,
        name: child.name,
        version: child.version,
        description: child.description.or(parent.description),
        category: child.category.or(parent.category),
        recommended_for: if child.recommended_for.is_empty() {
            parent.recommended_for
        } else {
            child.recommended_for
        },
        compatibility: child.compatibility.or(parent.compatibility),
        outputs,
        resources,
        requirements: ThemeRequirements {
            fonts,
            system_latex_packages: latex_packages,
        },
        metadata: ThemeMetadata { defaults: metadata },
        outputs_explicit: child.outputs_explicit,
        tokens: merge_tokens(parent.tokens, child.tokens),
        packages,
    }
}

fn validate_resolved_theme_outputs(theme: &ResolvedTheme) -> Result<()> {
    let mut unsupported = Vec::new();
    for output in &theme.outputs {
        let has_resource = match output.as_str() {
            "html" => {
                !theme.tokens.is_empty()
                    || !theme.resources.html_css.is_empty()
                    || theme.resources.html_template.is_some()
            }
            "epub" => {
                !theme.tokens.is_empty()
                    || !theme.resources.epub_css.is_empty()
                    || theme.resources.epub_template.is_some()
            }
            "pdf" | "latex" => {
                !theme.tokens.is_empty()
                    || !theme.resources.latex_packages.is_empty()
                    || !theme.resources.latex_headers.is_empty()
                    || theme.resources.latex_template.is_some()
            }
            "docx" => theme.resources.docx_reference_doc.is_some(),
            "pptx" => theme.resources.pptx_reference_doc.is_some(),
            _ => false,
        };
        if !has_resource {
            unsupported.push(output.clone());
        }
    }
    if unsupported.is_empty() {
        Ok(())
    } else {
        Err(OmniDocError::Config(format!(
            "resolved theme '{}@{}' declares output(s) {} but its inheritance chain has no matching resource",
            theme.id,
            theme.version,
            unsupported.join(", ")
        )))
    }
}

fn merge_tokens(parent: ThemeTokens, child: ThemeTokens) -> ThemeTokens {
    ThemeTokens {
        color: super::package::ThemeColorTokens {
            text: child.color.text.or(parent.color.text),
            background: child.color.background.or(parent.color.background),
            accent: child.color.accent.or(parent.color.accent),
            muted: child.color.muted.or(parent.color.muted),
            link: child.color.link.or(parent.color.link),
            border: child.color.border.or(parent.color.border),
            code_background: child.color.code_background.or(parent.color.code_background),
        },
        typography: super::package::ThemeTypographyTokens {
            body: child.typography.body.or(parent.typography.body),
            heading: child.typography.heading.or(parent.typography.heading),
            mono: child.typography.mono.or(parent.typography.mono),
            base_size_pt: child
                .typography
                .base_size_pt
                .or(parent.typography.base_size_pt),
            line_height: child
                .typography
                .line_height
                .or(parent.typography.line_height),
        },
        page: super::package::ThemePageTokens {
            size: child.page.size.or(parent.page.size),
            margin_top_mm: child.page.margin_top_mm.or(parent.page.margin_top_mm),
            margin_right_mm: child.page.margin_right_mm.or(parent.page.margin_right_mm),
            margin_bottom_mm: child.page.margin_bottom_mm.or(parent.page.margin_bottom_mm),
            margin_left_mm: child.page.margin_left_mm.or(parent.page.margin_left_mm),
        },
    }
}

pub fn materialize_theme_tokens(
    theme: &ResolvedTheme,
    project_root: &Path,
) -> Result<GeneratedThemeAssets> {
    if theme.tokens.is_empty() {
        return Ok(GeneratedThemeAssets::default());
    }
    let serialized =
        toml::to_string(&theme.tokens).map_err(|error| OmniDocError::Other(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(theme.id.as_bytes());
    hasher.update(theme.version.as_bytes());
    hasher.update(serialized.as_bytes());
    for package in &theme.packages {
        hasher.update(package.digest.as_bytes());
    }
    let digest = format!("{:x}", hasher.finalize());
    let stem = theme.id.replace(['/', '.'], "-");
    let directory = project_root
        .join(".omnidoc-cache")
        .join("themes")
        .join(format!("{}-{}", stem, &digest[..16]));
    fs::create_dir_all(&directory)?;
    let css = directory.join("tokens.css");
    let latex = directory.join("tokens.tex");
    write_if_changed(&css, render_css_tokens(&theme.tokens).as_bytes())?;
    write_if_changed(&latex, render_latex_tokens(&theme.tokens).as_bytes())?;
    Ok(GeneratedThemeAssets {
        css: Some(css),
        latex_header: Some(latex),
    })
}

fn render_css_tokens(tokens: &ThemeTokens) -> String {
    let mut declarations = Vec::new();
    for (name, value) in [
        ("text", tokens.color.text.as_deref()),
        ("background", tokens.color.background.as_deref()),
        ("accent", tokens.color.accent.as_deref()),
        ("muted", tokens.color.muted.as_deref()),
        ("link", tokens.color.link.as_deref()),
        ("border", tokens.color.border.as_deref()),
        ("code-background", tokens.color.code_background.as_deref()),
    ] {
        if let Some(value) = value {
            declarations.push(format!("  --omnidoc-color-{name}: {value};"));
        }
    }
    for (name, value) in [
        ("body", tokens.typography.body.as_deref()),
        ("heading", tokens.typography.heading.as_deref()),
        ("mono", tokens.typography.mono.as_deref()),
    ] {
        if let Some(value) = value {
            declarations.push(format!(
                "  --omnidoc-font-{name}: \"{}\";",
                css_escape(value)
            ));
        }
    }
    if let Some(value) = tokens.typography.base_size_pt {
        declarations.push(format!("  --omnidoc-base-size: {value}pt;"));
    }
    if let Some(value) = tokens.typography.line_height {
        declarations.push(format!("  --omnidoc-line-height: {value};"));
    }
    let mut output = String::from("/* Generated by OmniDoc theme API v1. */\n:root {\n");
    output.push_str(&declarations.join("\n"));
    output.push_str("\n}\n");
    output.push_str(
        "body { color: var(--omnidoc-color-text, inherit); background: var(--omnidoc-color-background, inherit); font-family: var(--omnidoc-font-body, inherit); font-size: var(--omnidoc-base-size, inherit); line-height: var(--omnidoc-line-height, inherit); }\n",
    );
    output.push_str(
        "h1, h2, h3, h4, h5, h6 { font-family: var(--omnidoc-font-heading, inherit); color: var(--omnidoc-color-accent, inherit); }\n",
    );
    output.push_str(
        "a { color: var(--omnidoc-color-link, var(--omnidoc-color-accent, inherit)); }\n",
    );
    output.push_str(
        "code, pre, kbd, samp { font-family: var(--omnidoc-font-mono, monospace); }\npre, code { background-color: var(--omnidoc-color-code-background, inherit); }\n",
    );
    let page = render_css_page(tokens);
    if !page.is_empty() {
        output.push_str("@page {\n");
        output.push_str(&page);
        output.push_str("}\n");
    }
    output
}

fn render_css_page(tokens: &ThemeTokens) -> String {
    let mut declarations = Vec::new();
    if let Some(size) = tokens.page.size.as_deref() {
        let size = match size.to_ascii_lowercase().as_str() {
            "a4" => "A4",
            "a5" => "A5",
            "letter" => "letter",
            _ => size,
        };
        declarations.push(format!("  size: {size};\n"));
    }
    for (name, value) in [
        ("top", tokens.page.margin_top_mm),
        ("right", tokens.page.margin_right_mm),
        ("bottom", tokens.page.margin_bottom_mm),
        ("left", tokens.page.margin_left_mm),
    ] {
        if let Some(value) = value {
            declarations.push(format!("  margin-{name}: {value}mm;\n"));
        }
    }
    declarations.concat()
}

fn render_latex_tokens(tokens: &ThemeTokens) -> String {
    let mut output = String::from("% Generated by OmniDoc theme API v1.\n");
    output.push_str("\\usepackage{xcolor}\n");
    for (name, value) in [
        ("Text", tokens.color.text.as_deref()),
        ("Background", tokens.color.background.as_deref()),
        ("Accent", tokens.color.accent.as_deref()),
        ("Muted", tokens.color.muted.as_deref()),
        ("Link", tokens.color.link.as_deref()),
        ("Border", tokens.color.border.as_deref()),
        ("CodeBackground", tokens.color.code_background.as_deref()),
    ] {
        if let Some(value) = value.and_then(normalized_hex_color) {
            output.push_str(&format!(
                "\\definecolor{{OmniTheme{name}}}{{HTML}}{{{value}}}\n"
            ));
        }
    }
    if tokens.color.text.is_some() {
        output.push_str("\\AtBeginDocument{\\color{OmniThemeText}}\n");
    }
    if let Some(body) = tokens.typography.body.as_deref() {
        output.push_str(&format!(
            "\\AtBeginDocument{{\\ifdefined\\setmainfont\\setmainfont{{{body}}}\\fi\\ifdefined\\setCJKmainfont\\setCJKmainfont{{{body}}}\\fi}}\n"
        ));
    }
    if let Some(heading) = tokens.typography.heading.as_deref() {
        output.push_str(&format!(
            "\\providecommand{{\\OmniThemeHeadingFont}}{{{heading}}}\n"
        ));
    }
    if let Some(mono) = tokens.typography.mono.as_deref() {
        output.push_str(&format!(
            "\\AtBeginDocument{{\\ifdefined\\setmonofont\\setmonofont{{{mono}}}\\fi\\ifdefined\\setCJKmonofont\\setCJKmonofont{{{mono}}}\\fi}}\n"
        ));
    }
    if let Some(size) = tokens.typography.base_size_pt {
        output.push_str(&format!(
            "\\providecommand{{\\OmniThemeBaseFontSize}}{{{size}pt}}\n"
        ));
    }
    if let Some(line_height) = tokens.typography.line_height {
        output.push_str(&format!("\\linespread{{{line_height}}}\n"));
    }
    let geometry = render_geometry_options(tokens);
    if !geometry.is_empty() {
        output.push_str("\\makeatletter\n");
        output.push_str(&format!(
            "\\@ifpackageloaded{{geometry}}{{\\geometry{{{geometry}}}}}{{\\usepackage[{geometry}]{{geometry}}}}\n"
        ));
        output.push_str("\\makeatother\n");
    }
    output
}

fn render_geometry_options(tokens: &ThemeTokens) -> String {
    let mut options = Vec::new();
    if let Some(size) = tokens.page.size.as_deref() {
        options.push(format!("{}paper", size.to_ascii_lowercase()));
    }
    for (name, value) in [
        ("top", tokens.page.margin_top_mm),
        ("right", tokens.page.margin_right_mm),
        ("bottom", tokens.page.margin_bottom_mm),
        ("left", tokens.page.margin_left_mm),
    ] {
        if let Some(value) = value {
            options.push(format!("{name}={value}mm"));
        }
    }
    options.join(",")
}

fn css_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn write_if_changed(path: &Path, content: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() == Some(content) {
        return Ok(());
    }
    crate::utils::fs::atomic_write(path, content)
}

fn resolve_resource_list(root: &Path, resources: &[String]) -> Vec<PathBuf> {
    resources
        .iter()
        .map(|resource| root.join(resource))
        .collect()
}

fn resolve_optional_resource(root: &Path, resource: &Option<String>) -> Option<PathBuf> {
    resource.as_ref().map(|resource| root.join(resource))
}

fn append_unique_paths(target: &mut Vec<PathBuf>, values: Vec<PathBuf>) {
    let mut seen = target.iter().cloned().collect::<BTreeSet<_>>();
    for value in values {
        if seen.insert(value.clone()) {
            target.push(value);
        }
    }
}

fn append_unique_strings(target: &mut Vec<String>, values: Vec<String>) {
    let mut seen = target
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    for value in values {
        if seen.insert(value.to_ascii_lowercase()) {
            target.push(value);
        }
    }
}

fn theme_outputs(theme: &ThemePackage) -> Vec<String> {
    if let Some(declared_outputs) = &theme.outputs {
        let mut outputs = declared_outputs
            .iter()
            .map(|output| normalized_output(output))
            .collect::<Vec<_>>();
        outputs.sort();
        outputs.dedup();
        return outputs;
    }
    let mut outputs = Vec::new();
    if !theme.resources.html_css.is_empty()
        || theme.resources.html_template.is_some()
        || !theme.tokens.is_empty()
    {
        outputs.push("html".to_string());
    }
    if !theme.resources.epub_css.is_empty()
        || theme.resources.epub_template.is_some()
        || !theme.tokens.is_empty()
    {
        outputs.push("epub".to_string());
    }
    if !theme.resources.latex_packages.is_empty()
        || !theme.resources.latex_headers.is_empty()
        || theme.resources.latex_template.is_some()
        || !theme.tokens.is_empty()
    {
        outputs.push("pdf".to_string());
        outputs.push("latex".to_string());
    }
    if theme.resources.docx_reference_doc.is_some() {
        outputs.push("docx".to_string());
    }
    if theme.resources.pptx_reference_doc.is_some() {
        outputs.push("pptx".to_string());
    }
    outputs
}

fn catalog_resources(resources: &ThemePackageResources) -> ThemeCatalogResources {
    ThemeCatalogResources {
        html_css: resources.html_css.clone(),
        epub_css: resources.epub_css.clone(),
        latex_packages: resources.latex_packages.clone(),
        latex_headers: resources.latex_headers.clone(),
        html_template: resources.html_template.clone(),
        epub_template: resources.epub_template.clone(),
        latex_template: resources.latex_template.clone(),
        docx_reference_doc: resources.docx_reference_doc.clone(),
        pptx_reference_doc: resources.pptx_reference_doc.clone(),
    }
}

fn normalized_requirements(requirements: &ThemePackageRequirements) -> ThemeRequirements {
    ThemeRequirements {
        fonts: requirements.fonts.clone(),
        system_latex_packages: requirements.system_latex_packages.clone(),
    }
}

fn normalized_metadata(metadata: &ThemePackageMetadata) -> ThemeMetadata {
    ThemeMetadata {
        defaults: metadata.defaults.clone(),
    }
}

fn builtin_theme_entries(config: &MergedConfig) -> Vec<ThemeCatalogEntry> {
    let library = library_root(config);
    let directory = library.join("themes");
    let Ok(entries) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut reports = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("toml"))
        .map(|path| builtin_theme_entry(&library, &path))
        .collect::<Vec<_>>();
    reports.sort_by(|left, right| left.id.cmp(&right.id));
    reports
}

fn builtin_theme_entry(library: &Path, path: &Path) -> ThemeCatalogEntry {
    match load_legacy_descriptor(library, path) {
        Ok(descriptor) => ThemeCatalogEntry {
            manifest_path: path.to_string_lossy().to_string(),
            id: descriptor.id.clone(),
            name: descriptor.name.clone(),
            version: descriptor.version.clone(),
            description: descriptor.description.clone(),
            category: descriptor.theme.category.clone(),
            recommended_for: descriptor.theme.recommended_for.clone(),
            compatibility: descriptor.theme.compatibility.clone(),
            compatible_omnidoc: Some(descriptor.compatible_omnidoc.clone()),
            compatible_pandoc: descriptor.compatible_pandoc.clone(),
            source: "builtin".to_string(),
            scope: PackageScope::Builtin,
            digest: Some(descriptor.digest.clone()),
            outputs: theme_outputs(&descriptor.theme),
            resources: catalog_resources(&descriptor.theme.resources),
            requirements: normalized_requirements(&descriptor.theme.requirements),
            metadata: normalized_metadata(&descriptor.theme.metadata),
            has_tokens: false,
            valid: true,
            errors: Vec::new(),
        },
        Err(error) => ThemeCatalogEntry {
            manifest_path: path.to_string_lossy().to_string(),
            id: path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("invalid-theme")
                .to_string(),
            name: "Invalid built-in theme".to_string(),
            version: "unknown".to_string(),
            description: None,
            category: None,
            recommended_for: Vec::new(),
            compatibility: None,
            compatible_omnidoc: None,
            compatible_pandoc: None,
            source: "builtin".to_string(),
            scope: PackageScope::Builtin,
            digest: None,
            outputs: Vec::new(),
            resources: ThemeCatalogResources::default(),
            requirements: ThemeRequirements::default(),
            metadata: ThemeMetadata::default(),
            has_tokens: false,
            valid: false,
            errors: vec![error.to_string()],
        },
    }
}

fn builtin_theme_descriptors(config: &MergedConfig) -> Result<Vec<ThemeDescriptor>> {
    let library = library_root(config);
    let directory = library.join("themes");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&directory)?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|extension| extension.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
        .iter()
        .map(|path| load_legacy_descriptor(&library, path))
        .collect()
}

fn load_legacy_descriptor(library: &Path, path: &Path) -> Result<ThemeDescriptor> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(OmniDocError::Other(format!(
            "built-in theme manifest is a symbolic link: {}",
            path.display()
        )));
    }
    let content = fs::read_to_string(path)?;
    let legacy: LegacyThemeManifest = toml::from_str(&content).map_err(|error| {
        OmniDocError::Other(format!("invalid built-in theme manifest: {error}"))
    })?;
    if legacy.manifest_version != 1 {
        return Err(OmniDocError::Other(format!(
            "unsupported built-in theme manifest_version {}",
            legacy.manifest_version
        )));
    }
    let filename = path.file_stem().and_then(|name| name.to_str());
    if filename != Some(legacy.name.as_str()) {
        return Err(OmniDocError::Other(format!(
            "built-in theme '{}' does not match manifest filename",
            legacy.name
        )));
    }
    Version::parse(&legacy.version)
        .map_err(|error| OmniDocError::Other(format!("invalid theme version: {error}")))?;
    let requirement = VersionReq::parse(&legacy.compatible_omnidoc).map_err(|error| {
        OmniDocError::Other(format!("invalid theme compatibility range: {error}"))
    })?;
    let installed = Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|error| OmniDocError::Other(error.to_string()))?;
    if !requirement.matches(&installed) {
        return Err(OmniDocError::Other(format!(
            "theme requires OmniDoc {}, installed {}",
            legacy.compatible_omnidoc, installed
        )));
    }
    let resources = ThemePackageResources {
        html_css: legacy.resources.html_css,
        epub_css: legacy.resources.epub_css,
        latex_packages: legacy.resources.latex_packages,
        latex_headers: legacy.resources.latex_headers,
        html_template: legacy.resources.html_template,
        epub_template: legacy.resources.epub_template,
        latex_template: legacy.resources.latex_template,
        docx_reference_doc: legacy.resources.docx_reference_doc,
        pptx_reference_doc: legacy.resources.pptx_reference_doc,
    };
    let mut tracked_files = vec![path.to_path_buf()];
    for relative in legacy
        .resources
        .templates
        .iter()
        .chain(resources.html_css.iter())
        .chain(resources.epub_css.iter())
        .chain(resources.latex_packages.iter())
        .chain(resources.latex_headers.iter())
    {
        let safe = safe_relative_path(relative).ok_or_else(|| {
            OmniDocError::Other(format!("unsafe built-in theme resource: {relative}"))
        })?;
        let resource = library.join(safe);
        if !resource.is_file() {
            return Err(OmniDocError::Other(format!(
                "missing built-in theme resource: {relative}"
            )));
        }
        tracked_files.push(resource);
    }
    for relative in [
        resources.html_template.as_ref(),
        resources.epub_template.as_ref(),
        resources.latex_template.as_ref(),
        resources.docx_reference_doc.as_ref(),
        resources.pptx_reference_doc.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        let safe = safe_relative_path(relative).ok_or_else(|| {
            OmniDocError::Other(format!("unsafe built-in theme resource: {relative}"))
        })?;
        let resource = library.join(safe);
        if !resource.is_file() {
            return Err(OmniDocError::Other(format!(
                "missing built-in theme resource: {relative}"
            )));
        }
        tracked_files.push(resource);
    }
    tracked_files.sort();
    tracked_files.dedup();
    let digest = digest_files(library, &tracked_files)?;
    Ok(ThemeDescriptor {
        manifest_path: path.to_path_buf(),
        id: legacy.name.clone(),
        name: legacy.name,
        version: legacy.version,
        description: legacy.description,
        compatible_omnidoc: legacy.compatible_omnidoc,
        compatible_pandoc: None,
        theme: ThemePackage {
            api_version: 1,
            category: legacy.category,
            recommended_for: legacy.recommended_for,
            compatibility: legacy.compatibility,
            extends: None,
            outputs: None,
            resources,
            requirements: legacy.requirements,
            metadata: legacy.metadata,
            tokens: ThemeTokens::default(),
        },
        root: library.to_path_buf(),
        scope: PackageScope::Builtin,
        source: "builtin".to_string(),
        digest,
        tracked_files,
    })
}

fn library_root(config: &MergedConfig) -> PathBuf {
    config
        .lib_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| crate::utils::directories::data_local_dir().map(|path| path.join("omnidoc")))
        .unwrap_or_else(|| PathBuf::from(".local/share/omnidoc"))
}

#[cfg(test)]
mod tests {
    use super::{
        materialize_theme_tokens, merge_tokens, render_css_tokens, render_geometry_options,
        resolve_theme_manifest, resolve_theme_request,
    };
    use crate::config::MergedConfig;
    use crate::extensions::package::{
        PackageScope, ThemeColorTokens, ThemePageTokens, ThemeTokens, ThemeTypographyTokens,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn package_path(store: &Path, id: &str) -> PathBuf {
        id.split('/')
            .fold(store.join("themes"), |path, segment| path.join(segment))
            .join("1.0.0")
    }

    fn project_package_path(project: &Path, id: &str) -> PathBuf {
        id.split('/')
            .fold(
                project.join(".omnidoc/extensions/themes"),
                |path, segment| path.join(segment),
            )
            .join("1.0.0")
    }

    fn write_theme_package(
        root: &Path,
        id: &str,
        description: &str,
        extends: Option<&str>,
        text: Option<&str>,
        accent: Option<&str>,
    ) {
        fs::create_dir_all(root.join("styles")).expect("theme styles");
        fs::write(
            root.join("styles/theme.css"),
            format!("/* {description} */\n"),
        )
        .expect("theme CSS");
        let extends = extends
            .map(|parent| format!("extends = \"{parent}\"\n"))
            .unwrap_or_default();
        let text = text
            .map(|value| format!("text = \"{value}\"\n"))
            .unwrap_or_default();
        let accent = accent
            .map(|value| format!("accent = \"{value}\"\n"))
            .unwrap_or_default();
        fs::write(
            root.join(super::super::package::PACKAGE_MANIFEST_FILE),
            format!(
                r#"manifest_version = 2
kind = "theme"
id = "{id}"
name = "{description}"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"

[theme]
api_version = 1
{extends}outputs = ["html", "pdf"]

[theme.resources]
html_css = ["styles/theme.css"]

[theme.tokens.color]
{text}{accent}"#
            ),
        )
        .expect("theme manifest");
    }

    #[test]
    fn child_theme_tokens_override_parent_fields_only() {
        let parent = ThemeTokens {
            color: ThemeColorTokens {
                text: Some("#111111".to_string()),
                accent: Some("#0055aa".to_string()),
                ..Default::default()
            },
            typography: ThemeTypographyTokens {
                body: Some("Parent Body".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let child = ThemeTokens {
            color: ThemeColorTokens {
                accent: Some("#ff0000".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let merged = merge_tokens(parent, child);
        assert_eq!(merged.color.text.as_deref(), Some("#111111"));
        assert_eq!(merged.color.accent.as_deref(), Some("#ff0000"));
        assert_eq!(merged.typography.body.as_deref(), Some("Parent Body"));
    }

    #[test]
    fn token_renderers_emit_portable_css_and_page_geometry() {
        let tokens = ThemeTokens {
            color: ThemeColorTokens {
                text: Some("#202124".to_string()),
                ..Default::default()
            },
            page: ThemePageTokens {
                size: Some("a4".to_string()),
                margin_top_mm: Some(20.0),
                ..Default::default()
            },
            ..Default::default()
        };
        let css = render_css_tokens(&tokens);
        assert!(css.contains("--omnidoc-color-text: #202124"));
        assert!(css.contains("@page {\n  size: A4;\n  margin-top: 20mm;\n}"));
        assert_eq!(render_geometry_options(&tokens), "a4paper,top=20mm");
    }

    #[test]
    fn explicit_theme_outputs_limit_where_the_theme_is_applied() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        let package = package_path(&store, "acme/html-only");
        fs::create_dir_all(package.join("styles")).expect("theme package");
        fs::write(package.join("styles/theme.css"), "body { color: #111; }\n").expect("theme CSS");
        fs::write(
            package.join(super::super::package::PACKAGE_MANIFEST_FILE),
            r##"manifest_version = 2
kind = "theme"
id = "acme/html-only"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"

[theme]
api_version = 1
outputs = ["html"]

[theme.resources]
html_css = ["styles/theme.css"]

[theme.tokens.color]
text = "#111111"
"##,
        )
        .expect("theme manifest");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let theme = resolve_theme_request(None, &config, "acme/html-only@=1.0.0")
            .expect("resolve HTML-only theme");
        assert!(theme.supports_output("html5"));
        assert!(!theme.supports_output("pdf"));
    }

    #[test]
    fn resolves_single_parent_inheritance_and_materializes_merged_tokens() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        write_theme_package(
            &package_path(&user_store, "acme/base"),
            "acme/base",
            "Base",
            None,
            Some("#111111"),
            None,
        );
        write_theme_package(
            &project_package_path(&project, "acme/child"),
            "acme/child",
            "Child",
            Some("acme/base@^1"),
            None,
            Some("#3366CC"),
        );
        let config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let theme = resolve_theme_request(Some(&project), &config, "acme/child@=1.0.0")
            .expect("inherited theme");
        assert_eq!(theme.packages.len(), 2);
        assert_eq!(theme.packages[0].id, "acme/base");
        assert_eq!(theme.packages[0].scope, PackageScope::User);
        assert_eq!(theme.packages[1].id, "acme/child");
        assert_eq!(theme.packages[1].scope, PackageScope::Project);
        assert_eq!(theme.resources.html_css.len(), 2);
        assert_eq!(theme.tokens.color.text.as_deref(), Some("#111111"));
        assert_eq!(theme.tokens.color.accent.as_deref(), Some("#3366CC"));

        let generated = materialize_theme_tokens(&theme, &project).expect("generated tokens");
        let css = fs::read_to_string(generated.css.expect("token CSS")).expect("read token CSS");
        assert!(css.contains("--omnidoc-color-text: #111111"));
        assert!(css.contains("--omnidoc-color-accent: #3366CC"));
        let latex = fs::read_to_string(generated.latex_header.expect("token LaTeX"))
            .expect("read token LaTeX");
        assert!(latex.contains("OmniThemeText"));
        assert!(latex.contains("OmniThemeAccent"));
    }

    #[test]
    fn explicit_child_outputs_replace_the_inherited_output_set() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        write_theme_package(
            &package_path(&store, "acme/base"),
            "acme/base",
            "Base",
            None,
            Some("#111111"),
            None,
        );
        let child = package_path(&store, "acme/child");
        write_theme_package(
            &child,
            "acme/child",
            "Child",
            Some("acme/base@=1.0.0"),
            None,
            Some("#3366CC"),
        );
        let manifest_path = child.join(super::super::package::PACKAGE_MANIFEST_FILE);
        let manifest = fs::read_to_string(&manifest_path)
            .expect("child manifest")
            .replace("outputs = [\"html\", \"pdf\"]", "outputs = [\"html\"]");
        fs::write(&manifest_path, manifest).expect("narrow child outputs");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let theme = resolve_theme_request(None, &config, "acme/child@=1.0.0")
            .expect("resolve narrowed child theme");
        assert!(theme.supports_output("html"));
        assert!(!theme.supports_output("pdf"));
    }

    #[test]
    fn explicitly_empty_child_outputs_disable_inherited_outputs() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        write_theme_package(
            &package_path(&store, "acme/base"),
            "acme/base",
            "Base",
            None,
            Some("#111111"),
            None,
        );
        let child = package_path(&store, "acme/child");
        fs::create_dir_all(&child).expect("child theme package");
        fs::write(
            child.join(super::super::package::PACKAGE_MANIFEST_FILE),
            r#"manifest_version = 2
kind = "theme"
id = "acme/child"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"

[theme]
api_version = 1
extends = "acme/base@=1.0.0"
outputs = []
"#,
        )
        .expect("child manifest");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let theme = resolve_theme_request(None, &config, "acme/child@=1.0.0")
            .expect("resolve disabled child outputs");
        assert!(theme.outputs.is_empty());
        assert!(!theme.supports_output("html"));
        assert!(!theme.supports_output("pdf"));
    }

    #[test]
    fn inherited_outputs_require_resources_in_the_resolved_chain() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        write_theme_package(
            &package_path(&store, "acme/base"),
            "acme/base",
            "Base",
            None,
            Some("#111111"),
            None,
        );
        let child = package_path(&store, "acme/child");
        fs::create_dir_all(&child).expect("child theme package");
        fs::write(
            child.join(super::super::package::PACKAGE_MANIFEST_FILE),
            r#"manifest_version = 2
kind = "theme"
id = "acme/child"
version = "1.0.0"
compatible_omnidoc = ">=1.8,<2"

[theme]
api_version = 1
extends = "acme/base@=1.0.0"
outputs = ["docx"]
"#,
        )
        .expect("child manifest");
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let error = resolve_theme_request(None, &config, "acme/child@=1.0.0")
            .expect_err("DOCX output without a reference document must fail");
        assert!(error.to_string().contains("no matching resource"));
        assert!(error.to_string().contains("docx"));
    }

    #[test]
    fn theme_inheritance_depth_is_bounded() {
        let workspace = tempfile::tempdir().expect("workspace");
        let store = workspace.path().join("store");
        for index in 0..=super::MAX_THEME_INHERITANCE_DEPTH {
            let id = format!("acme/depth-{index}");
            let parent = (index > 0).then(|| format!("acme/depth-{}@=1.0.0", index - 1));
            write_theme_package(
                &package_path(&store, &id),
                &id,
                &format!("Depth {index}"),
                parent.as_deref(),
                Some("#111111"),
                None,
            );
        }
        let config = MergedConfig {
            extension_path: Some(store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let request = format!("acme/depth-{}@=1.0.0", super::MAX_THEME_INHERITANCE_DEPTH);

        let error = resolve_theme_request(None, &config, &request)
            .expect_err("excessive theme inheritance must fail");
        assert!(error.to_string().contains("maximum depth"));
    }

    #[test]
    fn project_then_user_then_builtin_precedence_is_deterministic() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        let empty_store = workspace.path().join("empty-store");
        let library = workspace.path().join("library");
        fs::create_dir_all(&project).expect("project");
        fs::create_dir_all(library.join("themes")).expect("built-in themes");
        fs::create_dir_all(library.join("pandoc/css")).expect("built-in CSS");
        fs::write(library.join("pandoc/css/shared.css"), "/* builtin */\n").expect("built-in CSS");
        fs::write(
            library.join("themes/shared.toml"),
            r#"manifest_version = 1
name = "shared"
version = "1.0.0"
description = "Built-in"
compatible_omnidoc = ">=1.8,<2"

[resources]
html_css = ["pandoc/css/shared.css"]
"#,
        )
        .expect("built-in manifest");
        write_theme_package(
            &package_path(&user_store, "shared"),
            "shared",
            "User",
            None,
            Some("#222222"),
            None,
        );
        write_theme_package(
            &project_package_path(&project, "shared"),
            "shared",
            "Project",
            None,
            Some("#333333"),
            None,
        );
        let config = MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let project_theme =
            resolve_theme_request(Some(&project), &config, "shared@=1.0.0").expect("project theme");
        assert_eq!(project_theme.name, "Project");
        assert_eq!(project_theme.packages[0].scope, PackageScope::Project);

        let user_theme = resolve_theme_request(None, &config, "shared@=1.0.0").expect("user theme");
        assert_eq!(user_theme.name, "User");
        assert_eq!(user_theme.packages[0].scope, PackageScope::User);

        let builtin_config = MergedConfig {
            lib_path: Some(library.to_string_lossy().to_string()),
            extension_path: Some(empty_store.to_string_lossy().to_string()),
            ..Default::default()
        };
        let builtin =
            resolve_theme_request(None, &builtin_config, "shared@=1.0.0").expect("built-in theme");
        assert_eq!(builtin.name, "shared");
        assert_eq!(builtin.packages[0].scope, PackageScope::Builtin);
    }

    #[test]
    fn manifest_resolution_can_validate_shadowed_theme_packages() {
        let workspace = tempfile::tempdir().expect("workspace");
        let project = workspace.path().join("project");
        let user_store = workspace.path().join("user-store");
        fs::create_dir_all(&project).expect("project");
        let user_package = package_path(&user_store, "acme/shared");
        let project_package = project_package_path(&project, "acme/shared");
        write_theme_package(
            &user_package,
            "acme/shared",
            "User",
            None,
            Some("#111111"),
            None,
        );
        write_theme_package(
            &project_package,
            "acme/shared",
            "Project",
            None,
            Some("#222222"),
            None,
        );
        let config = MergedConfig {
            extension_path: Some(user_store.to_string_lossy().to_string()),
            ..Default::default()
        };

        let selected = resolve_theme_request(Some(&project), &config, "acme/shared@=1.0.0")
            .expect("project theme wins normal resolution");
        assert_eq!(selected.packages.last().unwrap().root, project_package);

        let shadowed = resolve_theme_manifest(
            Some(&project),
            &config,
            &user_package.join(super::super::package::PACKAGE_MANIFEST_FILE),
        )
        .expect("resolve shadowed user theme by manifest");
        assert_eq!(shadowed.packages.last().unwrap().root, user_package);
        assert_ne!(
            shadowed.packages.last().unwrap().digest,
            selected.packages.last().unwrap().digest
        );
    }
}
