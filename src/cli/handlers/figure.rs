use crate::cli::commands::FigureSubcommand;
use crate::cli::handlers::common::create_figure_service;
use crate::doc::services::FigureService;
use crate::error::{OmniDocError, Result};
use std::path::PathBuf;

/// Bitfield options for figure generation
#[derive(Clone)]
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

#[derive(Clone, Default)]
pub struct KicadOptions {
    pub black_and_white: bool,
    pub exclude_drawing_sheet: bool,
    pub pages: Option<String>,
}

struct BitfieldRequest {
    sources: Vec<String>,
    options: BitfieldOptions,
    format: String,
    force: bool,
    output: Option<String>,
}

impl Default for BitfieldOptions {
    fn default() -> Self {
        Self {
            vspace: None,
            hspace: None,
            lanes: None,
            bits: None,
            fontfamily: "sans-serif".to_string(),
            fontweight: "normal".to_string(),
            fontsize: 14,
            strokewidth: 1.0,
            beautify: false,
            json5: false,
            no_json5: false,
            compact: false,
            hflip: false,
            vflip: false,
            trim: None,
            uneven: false,
            legend: Vec::new(),
        }
    }
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
            force: subcommand_force,
            output,
        }) => {
            let legend = parse_bitfield_legend(&legend);
            handle_bitfield(BitfieldRequest {
                sources,
                options: BitfieldOptions {
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
                },
                format,
                force: force || subcommand_force,
                output,
            })
        }
        Some(FigureSubcommand::Drawio {
            sources,
            drawio,
            format,
            force: subcommand_force,
            output,
        }) => handle_drawio(sources, drawio, format, force || subcommand_force, output),
        Some(FigureSubcommand::Dot {
            sources,
            gradot,
            format,
            force: subcommand_force,
            output,
        }) => handle_dot(sources, gradot, format, force || subcommand_force, output),
        Some(FigureSubcommand::Plantuml {
            sources,
            plantuml,
            format,
            force: subcommand_force,
            output,
        }) => handle_plantuml(sources, plantuml, format, force || subcommand_force, output),
        Some(FigureSubcommand::Kicad {
            sources,
            kicad_cli,
            format,
            black_and_white,
            exclude_drawing_sheet,
            pages,
            force: subcommand_force,
            output,
        }) => handle_kicad(
            sources,
            kicad_cli,
            format,
            force || subcommand_force,
            output,
            KicadOptions {
                black_and_white,
                exclude_drawing_sheet,
                pages,
            },
        ),
        Some(FigureSubcommand::Convert {
            sources,
            inkscape,
            imagemagick,
            format,
            force: subcommand_force,
            output,
        }) => handle_convert(
            sources,
            inkscape,
            imagemagick,
            format,
            force || subcommand_force,
            output,
        ),
        None => {
            // 自动检测文件类型
            if sources.is_empty() {
                return Err(OmniDocError::Project(
                    "No source files specified".to_string(),
                ));
            }
            handle_auto_detect(sources, format, force, output)
        }
    }
}

/// Helper function to prepare paths and call service
fn execute_figure_generation(
    sources: Vec<String>,
    output: Option<String>,
    format: String,
    force: bool,
    service: &FigureService,
    bitfield_options: Option<BitfieldOptions>,
) -> Result<()> {
    use std::env;
    let source_paths: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    // Use current working directory as project_path, not the parent of the source file
    // This prevents path duplication when source files are relative paths
    let project_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let output_dir = output.as_ref().map(PathBuf::from);

    service.generate_figures(
        &project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        bitfield_options.as_ref(),
    )
}

fn handle_bitfield(request: BitfieldRequest) -> Result<()> {
    let figure_service = create_figure_service(vec![])?;

    execute_figure_generation(
        request.sources,
        request.output,
        request.format,
        request.force,
        &figure_service,
        Some(request.options),
    )?;

    println!("✓ Bitfield generation completed");
    Ok(())
}

fn parse_bitfield_legend(legend: &[String]) -> Vec<(String, String)> {
    legend
        .iter()
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

fn handle_drawio(
    sources: Vec<String>,
    drawio: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let figure_service = create_figure_service(vec![("drawio", drawio)])?;

    execute_figure_generation(sources, output, format, force, &figure_service, None)?;

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
    let figure_service = create_figure_service(vec![("dot", gradot)])?;

    execute_figure_generation(sources, output, format, force, &figure_service, None)?;

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
    let figure_service = create_figure_service(vec![("plantuml", plantuml)])?;

    execute_figure_generation(sources, output, format, force, &figure_service, None)?;

    println!("✓ PlantUML generation completed");
    Ok(())
}

fn handle_kicad(
    sources: Vec<String>,
    kicad_cli: Option<String>,
    format: String,
    force: bool,
    output: Option<String>,
    options: KicadOptions,
) -> Result<()> {
    let figure_service = create_figure_service(vec![("kicad-cli", kicad_cli)])?;
    let source_paths: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    let project_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let output_dir = output.as_ref().map(PathBuf::from);
    figure_service.generate_kicad_figures(
        &project_path,
        &source_paths,
        output_dir.as_deref(),
        &format,
        force,
        &options,
    )?;
    println!("✓ KiCad schematic export completed");
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
    let figure_service =
        create_figure_service(vec![("inkscape", inkscape), ("imagemagick", imagemagick)])?;

    execute_figure_generation(sources, output, format, force, &figure_service, None)?;

    println!("✓ Image conversion completed");
    Ok(())
}

fn handle_auto_detect(
    sources: Vec<String>,
    format: String,
    force: bool,
    output: Option<String>,
) -> Result<()> {
    let figure_service = create_figure_service(vec![])?;

    execute_figure_generation(sources, output, format, force, &figure_service, None)?;

    println!("✓ Figure generation completed");
    Ok(())
}
