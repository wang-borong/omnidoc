use crate::build::executor::BuildExecutor;
use crate::config::MergedConfig;
use crate::doc::services::bitfield::render_bitfield_from_json;
use crate::error::{OmniDocError, Result};
use crate::utils::fs;
use std::path::{Path, PathBuf};

/// Figure generation service
/// Handles generation of various figure types (bitfield, drawio, dot, plantuml, image conversion)
pub struct FigureService {
    executor: BuildExecutor,
    config: MergedConfig,
}

impl FigureService {
    pub fn new(config: MergedConfig) -> Result<Self> {
        let executor = BuildExecutor::new(config.tool_paths.clone());
        Ok(Self { executor, config })
    }

    /// Generate figures from source files
    pub fn generate_figures(
        &self,
        project_path: &Path,
        sources: &[PathBuf],
        output_dir: Option<&Path>,
        format: Option<&str>,
        force: bool,
        bitfield_options: Option<&crate::cli::handlers::BitfieldOptions>,
    ) -> Result<()> {
        // Determine output directory
        let output_path = output_dir
            .map(|p| p.to_path_buf())
            .or_else(|| {
                self.config
                    .figure_output
                    .as_ref()
                    .map(|s| project_path.join(s))
            })
            .unwrap_or_else(|| project_path.join("figures"));

        // Create output directory if it doesn't exist
        if !fs::exists(&output_path) {
            fs::create_dir_all(&output_path)?;
        }

        // Determine output format
        let output_format = format.unwrap_or("pdf");

        // Process each source file
        for source in sources {
            let source_path = if source.is_absolute() {
                source.clone()
            } else {
                project_path.join(source)
            };

            if !fs::exists(&source_path) {
                return Err(OmniDocError::Project(format!(
                    "Source file not found: {}",
                    source_path.display()
                )));
            }

            // Detect file type from extension
            let file_type = detect_file_type(&source_path)?;

            match file_type {
                FigureType::Bitfield => {
                    self.generate_bitfield(
                        &source_path,
                        &output_path,
                        output_format,
                        force,
                        bitfield_options,
                    )?;
                }
                FigureType::Drawio => {
                    self.generate_drawio(&source_path, &output_path, output_format, force)?;
                }
                FigureType::Dot => {
                    self.generate_dot(&source_path, &output_path, output_format, force)?;
                }
                FigureType::Plantuml => {
                    self.generate_plantuml(&source_path, &output_path, output_format, force)?;
                }
                FigureType::Image => {
                    self.convert_image(&source_path, &output_path, output_format, force)?;
                }
                FigureType::Unknown => {
                    return Err(OmniDocError::Project(format!(
                        "Unknown file type: {}",
                        source_path.display()
                    )));
                }
            }
        }

        Ok(())
    }

    fn generate_bitfield(
        &self,
        source: &Path,
        output_dir: &Path,
        format: &str,
        force: bool,
        options: Option<&crate::cli::handlers::BitfieldOptions>,
    ) -> Result<()> {
        // Use default options if not provided
        let bitfield_options = options
            .cloned()
            .unwrap_or_else(|| crate::cli::handlers::BitfieldOptions::default());

        // Generate SVG from JSON
        let svg_content = render_bitfield_from_json(source, &bitfield_options)
            .map_err(|e| OmniDocError::Other(format!("Failed to render bitfield: {}", e)))?;

        // Determine output file path
        let output_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OmniDocError::Other("Invalid source file name".to_string()))?;

        let svg_output = output_dir.join(format!("{}.svg", output_name));
        let final_output = if format == "svg" {
            svg_output.clone()
        } else {
            output_dir.join(format!("{}.{}", output_name, format))
        };

        // Check if output exists and force flag
        if fs::exists(&final_output) && !force {
            return Ok(());
        }

        // Write SVG file
        fs::write(&svg_output, svg_content.as_bytes())?;

        // Convert to target format if needed
        if format != "svg" {
            self.convert_svg_to_format(&svg_output, &final_output, format)?;
        }

        Ok(())
    }

    fn generate_drawio(
        &self,
        source: &Path,
        output_dir: &Path,
        format: &str,
        force: bool,
    ) -> Result<()> {
        // Export each page as a separate file, matching Python tool behavior
        let drawio_path = self.executor.check_tool("drawio")?;

        // Determine base name and diagram names
        let base_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OmniDocError::Other("Invalid source file name".to_string()))?;

        let content = crate::utils::fs::read_to_string(source)?;
        let diagram_lines: Vec<&str> = content.lines().filter(|l| l.contains("<diagram")).collect();

        let mut names: Vec<String> = Vec::new();
        if diagram_lines.len() > 1 {
            for (idx, line) in diagram_lines.iter().enumerate() {
                let mut suffix = String::new();
                if let Some(start) = line.find("name=\"") {
                    let name_start = start + 6; // len of 'name="'
                    if let Some(rest) = line.get(name_start..) {
                        if let Some(end) = rest.find('"') {
                            let raw = &rest[..end];
                            if !raw.is_empty() {
                                suffix.push('-');
                                suffix.push_str(raw);
                            }
                        }
                    }
                }
                if suffix.is_empty() {
                    suffix = format!("-page-{}", idx + 1);
                }
                names.push(suffix);
            }
        } else if diagram_lines.len() == 1 {
            names.push(String::new());
        }

        // Fallback: if no <diagram> tags found, assume single page
        if names.is_empty() {
            names.push(String::new());
        }

        for (index, name_suffix) in names.iter().enumerate() {
            let output_file = output_dir.join(format!("{}{}.{}", base_name, name_suffix, format));
            if crate::utils::fs::exists(&output_file) && !force {
                continue;
            }

            let mut args: Vec<String> = vec![
                "--export".to_string(),
                "--format".to_string(),
                format.to_string(),
                "--page-index".to_string(),
                index.to_string(),
            ];
            if format.eq_ignore_ascii_case("pdf") {
                args.push("--crop".to_string());
            }
            args.push("-o".to_string());
            let output_str = output_file
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?;
            args.push(output_str.to_string());
            let source_str = source
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid source path".to_string()))?;
            args.push(source_str.to_string());

            let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            // Call drawio directly; ignore any GUI-related stderr noise as long as outputs are produced
            self.executor.execute(&drawio_path, &args_str, false)?;
        }

        Ok(())
    }

    fn generate_dot(
        &self,
        source: &Path,
        output_dir: &Path,
        format: &str,
        force: bool,
    ) -> Result<()> {
        let output_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OmniDocError::Other("Invalid source file name".to_string()))?;

        let output = output_dir.join(format!("{}.{}", output_name, format));

        if fs::exists(&output) && !force {
            return Ok(());
        }

        // Build dot command
        let dot_path = self.executor.check_tool("dot")?;
        let mut args = vec!["-T", format, "-o"];
        args.push(
            output
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?,
        );
        args.push(
            source
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid source path".to_string()))?,
        );

        let args_str: Vec<&str> = args.iter().map(|s| *s).collect();
        self.executor.execute(&dot_path, &args_str, false)?;

        Ok(())
    }

    fn generate_plantuml(
        &self,
        source: &Path,
        output_dir: &Path,
        format: &str,
        force: bool,
    ) -> Result<()> {
        let output_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OmniDocError::Other("Invalid source file name".to_string()))?;

        let output = output_dir.join(format!("{}.{}", output_name, format));

        if fs::exists(&output) && !force {
            return Ok(());
        }

        // Build plantuml command
        let plantuml_path = self.executor.check_tool("plantuml")?;
        let mut args = vec!["-t", format, "-o"];
        args.push(
            output_dir
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid output directory".to_string()))?,
        );
        args.push(
            source
                .to_str()
                .ok_or_else(|| OmniDocError::Other("Invalid source path".to_string()))?,
        );

        let args_str: Vec<&str> = args.iter().map(|s| *s).collect();
        self.executor.execute(&plantuml_path, &args_str, false)?;

        Ok(())
    }

    fn convert_image(
        &self,
        source: &Path,
        output_dir: &Path,
        format: &str,
        force: bool,
    ) -> Result<()> {
        let output_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OmniDocError::Other("Invalid source file name".to_string()))?;

        let output = output_dir.join(format!("{}.{}", output_name, format));

        if fs::exists(&output) && !force {
            return Ok(());
        }

        // Try to use imagemagick (convert command)
        if let Ok(convert_path) = self.executor.check_tool("convert") {
            let args = vec![
                source
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid source path".to_string()))?,
                output
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?,
            ];
            self.executor.execute(&convert_path, &args, false)?;
            return Ok(());
        }

        // Try to use inkscape for SVG conversion
        if let Ok(inkscape_path) = self.executor.check_tool("inkscape") {
            let args = vec![
                "--export-type",
                format,
                "--export-filename",
                output
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?,
                source
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid source path".to_string()))?,
            ];
            self.executor.execute(&inkscape_path, &args, false)?;
            return Ok(());
        }

        Err(OmniDocError::Other(
            "No image conversion tool found (imagemagick or inkscape)".to_string(),
        ))
    }

    fn convert_svg_to_format(
        &self,
        svg_path: &Path,
        output_path: &Path,
        format: &str,
    ) -> Result<()> {
        // Try inkscape first (better for SVG)
        if let Ok(inkscape_path) = self.executor.check_tool("inkscape") {
            let args = vec![
                "--export-type",
                format,
                "--export-filename",
                output_path
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?,
                svg_path
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid SVG path".to_string()))?,
            ];
            self.executor.execute(&inkscape_path, &args, false)?;
            return Ok(());
        }

        // Fallback to imagemagick
        if let Ok(convert_path) = self.executor.check_tool("convert") {
            let args = vec![
                svg_path
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid SVG path".to_string()))?,
                output_path
                    .to_str()
                    .ok_or_else(|| OmniDocError::Other("Invalid output path".to_string()))?,
            ];
            self.executor.execute(&convert_path, &args, false)?;
            return Ok(());
        }

        Err(OmniDocError::Other(
            "No SVG conversion tool found (inkscape or imagemagick)".to_string(),
        ))
    }
}

enum FigureType {
    Bitfield,
    Drawio,
    Dot,
    Plantuml,
    Image,
    Unknown,
}

fn detect_file_type(path: &Path) -> Result<FigureType> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| OmniDocError::Other("File has no extension".to_string()))?;

    match ext.as_str() {
        "json" | "json5" => {
            // Check if it's a bitfield JSON by reading a small portion
            if let Ok(content) = fs::read_to_string(path) {
                // Simple heuristic: check if it looks like a bitfield JSON array
                let trimmed = content.trim();
                if trimmed.starts_with('[') && trimmed.contains("bits") {
                    return Ok(FigureType::Bitfield);
                }
            }
            Ok(FigureType::Unknown)
        }
        "drawio" | "xml" => {
            // Check if it's a drawio XML file
            if let Ok(content) = fs::read_to_string(path) {
                if content.contains("draw.io") || content.contains("mxfile") {
                    return Ok(FigureType::Drawio);
                }
            }
            Ok(FigureType::Unknown)
        }
        "dot" | "gv" => Ok(FigureType::Dot),
        "puml" | "plantuml" => Ok(FigureType::Plantuml),
        "svg" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "webp" => {
            Ok(FigureType::Image)
        }
        _ => Ok(FigureType::Unknown),
    }
}
