use crate::cli::commands::FigureSubcommand;
use crate::config::{CliOverrides, ConfigManager};
use crate::doc::services::FigureService;
use crate::error::{OmniDocError, Result};
use std::path::{Path, PathBuf};

/// Bitfield options for figure generation
#[derive(Default, Clone)]
pub struct BitfieldOptions {
    pub vspace: Option<u32>,
    pub hspace: Option<u32>,
    pub lanes: Option<u32>,
    pub bits: Option<u32>,
    pub fontfamily: String,
    pub fontweight: String,
    pub fontsize: u32,
    pub strokewidth: f32,
    pub beautify: bool,
    pub json5: bool,
    pub no_json5: bool,
    pub compact: bool,
    pub hflip: bool,
    pub vflip: bool,
    pub trim: Option<f32>,
    pub uneven: bool,
    pub legend: Vec<(String, String)>,
}

/// Handle the 'figure' command
pub fn handle_figure(
    subcommand: Option<FigureSubcommand>,
    format: String,
    force: bool,
    output: Option<String>,
    sources: Vec<String>,
) -> Result<()> {
    match subcommand {
        Some(FigureSubcommand::Bitfield {
            sources,
            vspace,
            hspace,
            lanes,
            bits,
            fontfamily,
            fontweight,
            fontsize,
            strokewidth,
            beautify,
            json5,
            no_json5,
            compact,
            hflip,
            vflip,
            trim,
            uneven,
            legend,
            format,
            force,
            output,
        }) => {
            handle_bitfield(
                sources,
                vspace,
                hspace,
                lanes,
                bits,
                fontfamily,
                fontweight,
                fontsize,
                strokewidth,
                beautify,
                json5,
                no_json5,
                compact,
                hflip,
                vflip,
                trim,
                uneven,
                legend,
                format,
                force,
                output,
            )
        }
        Some(FigureSubcommand::Drawio {
            sources,
            drawio,
            format,
            force,
            output,
        }) => {
            handle_drawio(sources, drawio, format, force, output)
        }
        Some(FigureSubcommand::Dot {
            sources,
            gradot,
            format,
            force,
            output,
        }) => {
            handle_dot(sources, gradot, format, force, output)
        }
        Some(FigureSubcommand::Plantuml {
            sources,
            plantuml,
            format,
            force,
            output,
        }) => {
            handle_plantuml(sources, plantuml, format, force, output)
        }
        Some(FigureSubcommand::Convert {
            sources,
            inkscape,
            imagemagick,
            format,
            force,
            output,
        }) => {
            handle_convert(sources, inkscape, imagemagick, format, force, output)
        }
        None => {
            // 自动检测文件类型
            if sources.is_empty() {
                return Err(OmniDocError::Project("No source files specified".to_string()));
            }
            handle_auto_detect(sources, format, force, output)
        }
    }
}

fn handle_bitfield(
    sources: Vec<String>,
    vspace: Option<u32>,
    hspace: Option<u32>,
    lanes: Option<u32>,
    bits: Option<u32>,
    fontfamily: String,
    fontweight: String,
    fontsize: u32,
    strokewidth: f32,
    beautify: bool,
    json5: bool,
    no_json5: bool,
    compact: bool,
    hflip: bool,
    vflip: bool,
    trim: Option<f32>,
    uneven: bool,
    legend: Vec<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let config_manager = ConfigManager::new(None, CliOverrides::new())
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let merged_config = config_manager.get_merged().clone();
    let figure_service = FigureService::new(merged_config)?;

    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    let legend_parsed: Vec<(String, String)> = legend
        .iter()
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect();

    let bitfield_options = BitfieldOptions {
        vspace,
        hspace,
        lanes,
        bits,
        fontfamily,
        fontweight,
        fontsize,
        strokewidth,
        beautify,
        json5,
        no_json5,
        compact,
        hflip,
        vflip,
        trim,
        uneven,
        legend: legend_parsed,
    };

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        Some(&bitfield_options),
    )?;

    println!("✓ Bitfield generation completed");
    Ok(())
}

fn handle_drawio(
    sources: Vec<String>,
    drawio: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let mut cli_overrides = CliOverrides::new();
    if let Some(ref d) = drawio {
        cli_overrides = cli_overrides.with_tool_path("drawio".to_string(), Some(d.clone()));
    }

    let config_manager = ConfigManager::new(None, cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let mut merged_config = config_manager.get_merged().clone();
    if let Some(ref d) = drawio {
        merged_config.tool_paths.insert("drawio".to_string(), Some(d.clone()));
    }

    let figure_service = FigureService::new(merged_config)?;
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        None,
    )?;

    println!("✓ Drawio generation completed");
    Ok(())
}

fn handle_dot(
    sources: Vec<String>,
    gradot: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let mut cli_overrides = CliOverrides::new();
    if let Some(ref g) = gradot {
        cli_overrides = cli_overrides.with_tool_path("dot".to_string(), Some(g.clone()));
    }

    let config_manager = ConfigManager::new(None, cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let mut merged_config = config_manager.get_merged().clone();
    if let Some(ref g) = gradot {
        merged_config.tool_paths.insert("dot".to_string(), Some(g.clone()));
    }

    let figure_service = FigureService::new(merged_config)?;
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        None,
    )?;

    println!("✓ Dot generation completed");
    Ok(())
}

fn handle_plantuml(
    sources: Vec<String>,
    plantuml: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let mut cli_overrides = CliOverrides::new();
    if let Some(ref p) = plantuml {
        cli_overrides = cli_overrides.with_tool_path("plantuml".to_string(), Some(p.clone()));
    }

    let config_manager = ConfigManager::new(None, cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let mut merged_config = config_manager.get_merged().clone();
    if let Some(ref p) = plantuml {
        merged_config.tool_paths.insert("plantuml".to_string(), Some(p.clone()));
    }

    let figure_service = FigureService::new(merged_config)?;
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        None,
    )?;

    println!("✓ PlantUML generation completed");
    Ok(())
}

fn handle_convert(
    sources: Vec<String>,
    inkscape: Option<String>,
    imagemagick: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let mut cli_overrides = CliOverrides::new();
    if let Some(ref i) = inkscape {
        cli_overrides = cli_overrides.with_tool_path("inkscape".to_string(), Some(i.clone()));
    }
    if let Some(ref m) = imagemagick {
        cli_overrides = cli_overrides.with_tool_path("imagemagick".to_string(), Some(m.clone()));
    }

    let config_manager = ConfigManager::new(None, cli_overrides)
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let mut merged_config = config_manager.get_merged().clone();
    if let Some(ref i) = inkscape {
        merged_config.tool_paths.insert("inkscape".to_string(), Some(i.clone()));
    }
    if let Some(ref m) = imagemagick {
        merged_config.tool_paths.insert("imagemagick".to_string(), Some(m.clone()));
    }

    let figure_service = FigureService::new(merged_config)?;
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        None,
    )?;

    println!("✓ Image conversion completed");
    Ok(())
}

fn handle_auto_detect(
    sources: Vec<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let config_manager = ConfigManager::new(None, CliOverrides::new())
        .map_err(|e| OmniDocError::Config(format!("Failed to load config: {}", e)))?;
    let merged_config = config_manager.get_merged().clone();
    let figure_service = FigureService::new(merged_config)?;

    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    let project_path = source_paths.first()
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    figure_service.generate_figures(
        project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        None,
    )?;

    println!("✓ Figure generation completed");
    Ok(())
}

