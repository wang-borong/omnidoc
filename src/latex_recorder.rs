use crate::error::{OmniDocError, Result};
use crate::terminal;
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
    wrapper_name: &str,
    depfile: &Path,
) -> Result<Option<RecorderInvocation>> {
    let wrapper_file = if cfg!(windows) {
        format!("{wrapper_name}.exe")
    } else {
        wrapper_name.to_string()
    };
    let engine_name = OsStr::new(&wrapper_file);
    if !supports_recorder_engine(engine_name) {
        return Ok(None);
    }

    let directory = project_path.join(".omnidoc-cache/latex-recorder-bin");
    fs::create_dir_all(&directory)?;
    let wrapper = directory.join(engine_name);
    if wrapper.exists() || wrapper.is_symlink() {
        fs::remove_file(&wrapper)?;
    }
    if depfile.exists() {
        fs::remove_file(depfile)?;
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
    let invoked_as = std::env::args_os().next()?;
    if !is_recorder_invocation(&invoked_as) {
        return None;
    }
    let invoked_name = Path::new(&invoked_as)
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let real_engine = std::env::var_os(REAL_ENGINE_ENV)?;
    let depfile = std::env::var_os(DEPFILE_ENV)?;
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    let status = match Command::new(&real_engine).args(&args).status() {
        Ok(status) => status,
        Err(error) => {
            terminal::print_error(&OmniDocError::CommandExecution(format!(
                "LaTeX recorder could not execute {}\n{}",
                Path::new(&real_engine).display(),
                error
            )));
            return Some(1);
        }
    };

    if status.success() {
        let recorded = if invoked_name == "tectonic" {
            locate_tectonic_rules(&args).map(|context| {
                write_depfile_from_tectonic_rules(
                    &context.rules,
                    Path::new(&depfile),
                    &context.working_directory,
                    &context.output_directory,
                    &context.search_paths,
                )
            })
        } else {
            locate_fls(&args).map(|(fls, output_directory)| {
                write_depfile_from_fls(&fls, Path::new(&depfile), &[output_directory])
            })
        };
        if let Some(Err(error)) = recorded {
            terminal::warning(format!(
                "LaTeX recorder could not write its dependency file\n{error}"
            ));
        }
    }
    Some(status.code().unwrap_or(1))
}

fn is_recorder_invocation(program: &OsStr) -> bool {
    Path::new(program)
        .file_name()
        .is_some_and(supports_recorder_engine)
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

    write_depfile(depfile, "latex-fls", &dependencies)?;
    Ok(dependencies.len())
}

pub fn write_depfile_from_tectonic_rules(
    rules_path: &Path,
    depfile: &Path,
    working_directory: &Path,
    output_directory: &Path,
    search_paths: &[PathBuf],
) -> Result<usize> {
    let content = fs::read_to_string(rules_path).map_err(|error| {
        OmniDocError::Other(format!(
            "cannot read Tectonic dependency rules {}: {error}",
            rules_path.display()
        ))
    })?;
    let reported_working_directory = working_directory.to_path_buf();
    let reported_output_directory = if output_directory.is_absolute() {
        output_directory.to_path_buf()
    } else {
        reported_working_directory.join(output_directory)
    };
    let working_directory = working_directory
        .canonicalize()
        .unwrap_or_else(|_| working_directory.to_path_buf());
    let output_directory = if output_directory.is_absolute() {
        output_directory.to_path_buf()
    } else {
        working_directory.join(output_directory)
    };
    let output_directory = output_directory.canonicalize().unwrap_or(output_directory);
    let search_paths = search_paths
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                working_directory.join(path)
            }
        })
        .collect::<Vec<_>>();
    let mut dependencies = BTreeSet::new();
    for dependency in parse_makefile_dependencies(&content) {
        let Some(canonical) = resolve_tectonic_dependency(
            &dependency,
            &working_directory,
            &reported_output_directory,
            &output_directory,
            &search_paths,
        ) else {
            continue;
        };
        if canonical.starts_with(&output_directory) || volatile_latex_output(&canonical) {
            continue;
        }
        dependencies.insert(canonical);
    }
    write_depfile(depfile, "tectonic-makefile", &dependencies)?;
    Ok(dependencies.len())
}

fn write_depfile(depfile: &Path, source: &str, dependencies: &BTreeSet<PathBuf>) -> Result<()> {
    if let Some(parent) = depfile.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut output = format!("# omnidoc-depfile-v1\n# source={source}\n");
    for dependency in dependencies {
        output.push_str(&dependency.to_string_lossy());
        output.push('\n');
    }
    let temporary = depfile.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&temporary, output)?;
    if depfile.exists() {
        fs::remove_file(depfile)?;
    }
    fs::rename(&temporary, depfile)?;
    Ok(())
}

fn parse_makefile_dependencies(content: &str) -> Vec<PathBuf> {
    let joined = content.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut dependencies = Vec::new();
    for line in joined.lines() {
        let Some((_, values)) = line.split_once(" : ") else {
            continue;
        };
        dependencies.extend(parse_makefile_words(values).into_iter().map(PathBuf::from));
    }
    dependencies
}

fn parse_makefile_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\'
            && characters
                .peek()
                .is_some_and(|next| next.is_whitespace() || *next == '#')
        {
            current.push(characters.next().expect("peeked Makefile escape"));
        } else if character.is_whitespace() {
            if !current.is_empty() {
                words.push(current.replace("$$", "$"));
                current = String::new();
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        words.push(current.replace("$$", "$"));
    }
    words
}

fn resolve_tectonic_dependency(
    dependency: &Path,
    working_directory: &Path,
    reported_output_directory: &Path,
    output_directory: &Path,
    search_paths: &[PathBuf],
) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if dependency.is_absolute() {
        candidates.push(dependency.to_path_buf());
    } else {
        candidates.push(working_directory.join(dependency));
    }

    let search_relative = dependency
        .strip_prefix(reported_output_directory)
        .or_else(|_| dependency.strip_prefix(output_directory))
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| (!dependency.is_absolute()).then(|| dependency.to_path_buf()));
    if let Some(relative) = search_relative {
        candidates.push(working_directory.join(&relative));
        for search_path in search_paths {
            candidates.push(search_path.join(&relative));
        }
    }
    if let Some(file_name) = dependency.file_name() {
        for search_path in search_paths {
            candidates.push(search_path.join(file_name));
        }
    }

    candidates.into_iter().find_map(|candidate| {
        candidate
            .is_file()
            .then(|| candidate.canonicalize().unwrap_or(candidate))
    })
}

struct TectonicRuleContext {
    rules: PathBuf,
    working_directory: PathBuf,
    output_directory: PathBuf,
    search_paths: Vec<PathBuf>,
}

fn locate_tectonic_rules(args: &[OsString]) -> Option<TectonicRuleContext> {
    let working_directory = std::env::current_dir().ok()?;
    let mut rules = None;
    let mut output_directory = None;
    let mut search_paths = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].to_string_lossy();
        if argument == "--makefile-rules" {
            index += 1;
            rules = args.get(index).map(PathBuf::from);
        } else if let Some(value) = argument.strip_prefix("--makefile-rules=") {
            rules = Some(PathBuf::from(value));
        } else if matches!(argument.as_ref(), "--outdir" | "-o") {
            index += 1;
            output_directory = args.get(index).map(PathBuf::from);
        } else if let Some(value) = argument.strip_prefix("--outdir=") {
            output_directory = Some(PathBuf::from(value));
        } else if let Some(value) = argument.strip_prefix("-Zsearch-path=") {
            search_paths.push(PathBuf::from(value));
        } else if argument == "-Z" {
            if let Some(value) = args
                .get(index + 1)
                .and_then(|value| value.to_str())
                .and_then(|value| value.strip_prefix("search-path="))
            {
                search_paths.push(PathBuf::from(value));
                index += 1;
            }
        }
        index += 1;
    }
    Some(TectonicRuleContext {
        rules: rules?,
        output_directory: output_directory.unwrap_or_else(|| PathBuf::from(".")),
        working_directory,
        search_paths,
    })
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
    matches!(
        stem.as_str(),
        "xelatex" | "pdflatex" | "lualatex" | "tectonic"
    )
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
    use super::{
        is_recorder_invocation, locate_fls, parse_makefile_words, write_depfile_from_fls,
        write_depfile_from_tectonic_rules,
    };
    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn recorder_mode_requires_a_latex_engine_invocation_name() {
        assert!(is_recorder_invocation(std::ffi::OsStr::new(
            "/tmp/wrappers/xelatex"
        )));
        assert!(is_recorder_invocation(std::ffi::OsStr::new("lualatex")));
        assert!(is_recorder_invocation(std::ffi::OsStr::new("tectonic")));
        assert!(!is_recorder_invocation(std::ffi::OsStr::new(
            "/usr/local/bin/omnidoc"
        )));
    }

    #[test]
    fn makefile_word_parser_preserves_windows_path_separators() {
        let words = parse_makefile_words(
            r"C:\work\main.tex C:\work\chapters\intro.tex escaped\ path\#1.tex",
        );

        assert_eq!(
            words,
            [
                r"C:\work\main.tex",
                r"C:\work\chapters\intro.tex",
                "escaped path#1.tex",
            ]
        );
    }

    #[test]
    fn resolves_tectonic_virtual_output_dependencies_against_search_paths() {
        let root = tempfile::tempdir().expect("Tectonic dependency root");
        let output = root.path().join("build");
        let packages = root.path().join("texmf/tex/common");
        fs::create_dir_all(&output).expect("output directory");
        fs::create_dir_all(&packages).expect("package directory");
        let entry = root.path().join("main.tex");
        let chapter = root.path().join("chapters/intro.tex");
        let package = packages.join("probe.sty");
        fs::write(&entry, "entry").expect("entry");
        fs::create_dir_all(chapter.parent().expect("chapter parent")).expect("chapter directory");
        fs::write(&chapter, "chapter").expect("chapter");
        fs::write(&package, "package").expect("package");
        let rules = root.path().join("tectonic.make");
        fs::write(
            &rules,
            format!(
                "{} : {} \\\n  {} \\\n  {}\n",
                output.join("main.pdf").display(),
                entry.display(),
                output.join("chapters/intro.tex").display(),
                output.join("probe.sty").display()
            ),
        )
        .expect("rules");
        let depfile = root.path().join("latex-inputs.d");

        write_depfile_from_tectonic_rules(
            &rules,
            &depfile,
            root.path(),
            &output,
            std::slice::from_ref(&packages),
        )
        .expect("Tectonic depfile");

        let content = fs::read_to_string(depfile).expect("depfile content");
        assert!(content.contains(
            &entry
                .canonicalize()
                .expect("canonical entry")
                .display()
                .to_string()
        ));
        assert!(content.contains(
            &chapter
                .canonicalize()
                .expect("canonical chapter")
                .display()
                .to_string()
        ));
        assert!(content.contains(
            &package
                .canonicalize()
                .expect("canonical package")
                .display()
                .to_string()
        ));
    }

    #[cfg(unix)]
    #[test]
    fn resolves_virtual_dependencies_through_a_noncanonical_directory_alias() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("Tectonic dependency root");
        let actual = root.path().join("actual");
        let alias = root.path().join("alias");
        let output = alias.join("build");
        let chapter = actual.join("chapters/intro.tex");
        fs::create_dir_all(actual.join("build")).expect("output directory");
        fs::create_dir_all(chapter.parent().expect("chapter parent")).expect("chapter directory");
        fs::write(&chapter, "chapter").expect("chapter");
        symlink(&actual, &alias).expect("working-directory alias");
        let rules = actual.join("tectonic.make");
        fs::write(
            &rules,
            format!(
                "{} : {}\n",
                output.join("main.pdf").display(),
                output.join("chapters/intro.tex").display()
            ),
        )
        .expect("rules");
        let depfile = actual.join("latex-inputs.d");

        write_depfile_from_tectonic_rules(&rules, &depfile, &alias, &output, &[])
            .expect("Tectonic depfile");

        let content = fs::read_to_string(depfile).expect("depfile content");
        assert!(content.contains(
            &chapter
                .canonicalize()
                .expect("canonical chapter")
                .display()
                .to_string()
        ));
    }

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
