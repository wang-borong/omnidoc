use crate::error::{OmniDocError, Result};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REAL_ENGINE_ENV: &str = "OMNIDOC_LATEX_RECORDER_ENGINE";
const DEPFILE_ENV: &str = "OMNIDOC_LATEX_RECORDER_DEPFILE";

pub struct RecorderInvocation {
    pub wrapper: PathBuf,
    pub environment: Vec<(OsString, OsString)>,
}

pub fn prepare_wrapper(
    project_path: &Path,
    real_engine: &Path,
    depfile: &Path,
) -> Result<Option<RecorderInvocation>> {
    let Some(engine_name) = real_engine.file_name() else {
        return Ok(None);
    };
    if !supports_recorder_engine(engine_name) {
        return Ok(None);
    }

    let directory = project_path.join(".omnidoc-cache/latex-recorder-bin");
    fs::create_dir_all(&directory)?;
    let wrapper = directory.join(engine_name);
    if wrapper.exists() || wrapper.is_symlink() {
        fs::remove_file(&wrapper)?;
    }
    let executable = std::env::current_exe()?;
    install_wrapper_executable(&executable, &wrapper)?;

    Ok(Some(RecorderInvocation {
        wrapper,
        environment: vec![
            (
                OsString::from(REAL_ENGINE_ENV),
                real_engine.as_os_str().to_os_string(),
            ),
            (
                OsString::from(DEPFILE_ENV),
                depfile.as_os_str().to_os_string(),
            ),
        ],
    }))
}

pub fn run_wrapper_from_env() -> Option<i32> {
    let real_engine = std::env::var_os(REAL_ENGINE_ENV)?;
    let depfile = std::env::var_os(DEPFILE_ENV)?;
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let status = match Command::new(&real_engine).args(&args).status() {
        Ok(status) => status,
        Err(error) => {
            eprintln!(
                "OmniDoc LaTeX recorder could not execute {}: {}",
                Path::new(&real_engine).display(),
                error
            );
            return Some(1);
        }
    };

    if status.success() {
        if let Some((fls, output_directory)) = locate_fls(&args) {
            if let Err(error) =
                write_depfile_from_fls(&fls, Path::new(&depfile), &[output_directory])
            {
                eprintln!("OmniDoc LaTeX recorder warning: {error}");
            }
        }
    }
    Some(status.code().unwrap_or(1))
}

pub fn write_depfile_from_fls(
    fls_path: &Path,
    depfile: &Path,
    excluded_roots: &[PathBuf],
) -> Result<usize> {
    let content = fs::read_to_string(fls_path).map_err(|error| {
        OmniDocError::Other(format!(
            "cannot read LaTeX recorder file {}: {error}",
            fls_path.display()
        ))
    })?;
    let working_directory = content
        .lines()
        .find_map(|line| line.strip_prefix("PWD "))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            fls_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });
    let excluded = excluded_roots
        .iter()
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.clone()))
        .collect::<Vec<_>>();
    let mut dependencies = BTreeSet::new();
    for input in content
        .lines()
        .filter_map(|line| line.strip_prefix("INPUT "))
    {
        let candidate = PathBuf::from(input);
        let candidate = if candidate.is_absolute() {
            candidate
        } else {
            working_directory.join(candidate)
        };
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        if !canonical.is_file()
            || excluded.iter().any(|root| canonical.starts_with(root))
            || volatile_latex_output(&canonical)
        {
            continue;
        }
        dependencies.insert(canonical);
    }

    if let Some(parent) = depfile.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::from("# omnidoc-depfile-v1\n# source=latex-fls\n");
    for dependency in &dependencies {
        output.push_str(&dependency.to_string_lossy());
        output.push('\n');
    }
    let temporary = depfile.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temporary, output)?;
    if depfile.exists() {
        fs::remove_file(depfile)?;
    }
    fs::rename(&temporary, depfile)?;
    Ok(dependencies.len())
}

fn locate_fls(args: &[OsString]) -> Option<(PathBuf, PathBuf)> {
    let mut output_directory = None;
    let mut jobname = None;
    let mut input = None;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        if argument == "-output-directory" || argument == "--output-directory" {
            index += 1;
            output_directory = args.get(index).map(PathBuf::from);
        } else if let Some(value) = argument
            .strip_prefix("-output-directory=")
            .or_else(|| argument.strip_prefix("--output-directory="))
        {
            output_directory = Some(PathBuf::from(value));
        } else if argument == "-jobname" || argument == "--jobname" {
            index += 1;
            jobname = args.get(index).cloned();
        } else if let Some(value) = argument
            .strip_prefix("-jobname=")
            .or_else(|| argument.strip_prefix("--jobname="))
        {
            jobname = Some(OsString::from(value));
        } else if Path::new(args[index].as_os_str())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("tex"))
        {
            input = Some(PathBuf::from(&args[index]));
        }
        index += 1;
    }

    let output_directory = output_directory.unwrap_or_else(|| PathBuf::from("."));
    let stem = jobname.or_else(|| input?.file_stem().map(|value| value.to_os_string()))?;
    Some((
        output_directory.join(stem).with_extension("fls"),
        output_directory,
    ))
}

fn supports_recorder_engine(engine_name: &OsStr) -> bool {
    let stem = Path::new(engine_name)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(stem.as_str(), "xelatex" | "pdflatex" | "lualatex")
}

fn volatile_latex_output(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aux"
                    | "bbl"
                    | "bcf"
                    | "blg"
                    | "fdb_latexmk"
                    | "fls"
                    | "log"
                    | "out"
                    | "run.xml"
                    | "synctex"
                    | "toc"
            )
        })
}

#[cfg(unix)]
fn install_wrapper_executable(executable: &Path, wrapper: &Path) -> Result<()> {
    std::os::unix::fs::symlink(executable, wrapper).map_err(OmniDocError::Io)
}

#[cfg(not(unix))]
fn install_wrapper_executable(executable: &Path, wrapper: &Path) -> Result<()> {
    fs::copy(executable, wrapper)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{locate_fls, write_depfile_from_fls};
    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn locates_pandoc_style_recorder_output() {
        let root = tempfile::tempdir().expect("recorder output directory");
        let output = root.path().join("render");
        let input = output.join("input.tex");
        let args = [
            OsString::from("-output-directory"),
            output.as_os_str().to_os_string(),
            OsString::from("-recorder"),
            input.as_os_str().to_os_string(),
        ];
        let (fls, directory) = locate_fls(&args).expect("recorder output");
        assert_eq!(directory, output);
        assert_eq!(fls, output.join("input.fls"));
    }

    #[test]
    fn normalizes_and_filters_fls_inputs() {
        let root = tempfile::tempdir().expect("recorder fixture");
        let output = root.path().join("build");
        let source = root.path().join("chapter.tex");
        let package = root.path().join("theme.sty");
        fs::create_dir_all(&output).expect("output directory");
        fs::write(&source, "chapter\n").expect("source");
        fs::write(&package, "package\n").expect("package");
        fs::write(output.join("book.aux"), "aux\n").expect("auxiliary");
        let fls = output.join("book.fls");
        fs::write(
            &fls,
            format!(
                "PWD {}\nINPUT {}\nINPUT {}\nINPUT {}\nINPUT {}\n",
                root.path().display(),
                source.display(),
                package.display(),
                package.display(),
                output.join("book.aux").display()
            ),
        )
        .expect("fls");
        let depfile = root.path().join(".omnidoc-cache/latex-inputs.d");
        let count =
            write_depfile_from_fls(&fls, &depfile, std::slice::from_ref(&output)).expect("depfile");
        assert_eq!(count, 2);
        let content = fs::read_to_string(depfile).expect("depfile content");
        let canonical_source = source
            .canonicalize()
            .expect("canonical source")
            .to_string_lossy()
            .to_string();
        let canonical_package = package
            .canonicalize()
            .expect("canonical package")
            .to_string_lossy()
            .to_string();
        assert!(content.contains(&canonical_source));
        assert!(content.contains(&canonical_package));
        assert!(!content.contains("book.aux"));
    }
}
