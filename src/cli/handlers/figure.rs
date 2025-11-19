use crate::cli::commands::FigureSubcommand;
use crate::cli::handlers::common::create_figure_service;
use crate::doc::services::FigureService;
use crate::error::{OmniDocError, Result};
use std::path::PathBuf;

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
            force: subcommand_force,
            output,
        }) => handle_bitfield(
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
            force || subcommand_force,
            output,
        ),
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
        Some(FigureSubcommand::Convert {
            sources,
            inkscape,
            imagemagick,
            format,
            force: subcommand_force,
            output,
        }) => handle_convert(sources, inkscape, imagemagick, format, force || subcommand_force, output),
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
    let source_paths: Vec<PathBuf> = sources.iter().map(|s| PathBuf::from(s)).collect();
    // Use current working directory as project_path, not the parent of the source file
    // This prevents path duplication when source files are relative paths
    let project_path = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let output_dir = output.as_ref().map(|s| PathBuf::from(s));

    service.generate_figures(
        &project_path,
        &source_paths,
        output_dir.as_deref(),
        Some(&format),
        force,
        bitfield_options.as_ref(),
    )
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
    let figure_service = create_figure_service(vec![])?;

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

    execute_figure_generation(
        sources,
        output,
        format,
        force,
        &figure_service,
        Some(bitfield_options),
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
