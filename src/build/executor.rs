use crate::constants::pandoc;
use crate::diagnostics::summarize_command_output;
use crate::error::{OmniDocError, Result};
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatexEnginePreference {
    Markdown,
    Latex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatexEngineKind {
    Tectonic,
    XeLatex,
    LuaLatex,
    PdfLatex,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatexEngineOrigin {
    Configured,
    Bundled,
    Path,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLatexEngine {
    pub executable: String,
    pub kind: LatexEngineKind,
    pub origin: LatexEngineOrigin,
}

impl ResolvedLatexEngine {
    pub fn is_tectonic(&self) -> bool {
        self.kind == LatexEngineKind::Tectonic
    }

    pub fn origin_label(&self) -> &'static str {
        match self.origin {
            LatexEngineOrigin::Configured => "configured",
            LatexEngineOrigin::Bundled => "bundled",
            LatexEngineOrigin::Path => "PATH",
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            LatexEngineKind::Tectonic => "tectonic",
            LatexEngineKind::XeLatex => "xelatex",
            LatexEngineKind::LuaLatex => "lualatex",
            LatexEngineKind::PdfLatex => "pdflatex",
            LatexEngineKind::Other => "other",
        }
    }

    pub fn recorder_program_name(&self) -> Option<&'static str> {
        match self.kind {
            LatexEngineKind::Tectonic => Some("tectonic"),
            LatexEngineKind::XeLatex => Some("xelatex"),
            LatexEngineKind::LuaLatex => Some("lualatex"),
            LatexEngineKind::PdfLatex => Some("pdflatex"),
            LatexEngineKind::Other => None,
        }
    }
}

/// 构建执行器
/// 负责工具检查和命令执行
pub struct BuildExecutor {
    tool_paths: std::collections::HashMap<String, Option<String>>,
}

impl BuildExecutor {
    pub fn new(tool_paths: std::collections::HashMap<String, Option<String>>) -> Self {
        Self { tool_paths }
    }

    /// 检查工具是否存在
    pub fn check_tool(&self, tool: &str) -> Result<String> {
        if tool == "latex_engine" {
            return self
                .resolve_latex_engine(LatexEnginePreference::Markdown)
                .map(|engine| engine.executable);
        }
        if tool == "tectonic" {
            return self.resolve_tectonic().map(|engine| engine.executable);
        }

        // 首先检查配置的路径
        if let Some(Some(path)) = self.tool_paths.get(tool) {
            if PathBuf::from(path).exists() {
                return Ok(path.clone());
            }
            if let Ok(resolved) = which::which(path) {
                return Ok(resolved.to_string_lossy().to_string());
            }

            return Err(OmniDocError::Other(format!(
                "Configured tool '{}' for '{}' not found. Please install it or update the configured path.",
                path, tool
            )));
        }

        // 检查系统 PATH
        if let Ok(path) = which::which(tool) {
            return Ok(path.to_string_lossy().to_string());
        }

        Err(OmniDocError::Other(format!(
            "Tool '{}' not found. Please install it or configure the path in config file.",
            tool
        )))
    }

    pub fn resolve_latex_engine(
        &self,
        preference: LatexEnginePreference,
    ) -> Result<ResolvedLatexEngine> {
        if let Some(Some(configured)) = self.tool_paths.get("latex_engine") {
            let configured = configured.trim();
            if !configured.is_empty() && !configured.eq_ignore_ascii_case("auto") {
                if configured.eq_ignore_ascii_case("tectonic") {
                    return self.resolve_tectonic().map(|mut engine| {
                        engine.origin = LatexEngineOrigin::Configured;
                        engine
                    });
                }
                return self.resolve_engine_program(configured, LatexEngineOrigin::Configured);
            }
        }

        match preference {
            LatexEnginePreference::Markdown => {
                if self.tool_paths.get("tectonic").is_some_and(|value| {
                    value.as_deref().is_some_and(|path| !path.trim().is_empty())
                }) {
                    return self.resolve_tectonic();
                }
                match self.resolve_tectonic() {
                    Ok(engine) => Ok(engine),
                    Err(tectonic_error) => self
                        .resolve_engine_program(
                            pandoc::DEFAULT_ENGINE_LATEX,
                            LatexEngineOrigin::Path,
                        )
                        .map_err(|xelatex_error| {
                            OmniDocError::Other(format!(
                                "No usable Markdown PDF engine was found. Tectonic: {tectonic_error} XeLaTeX: {xelatex_error}"
                            ))
                        }),
                }
            }
            LatexEnginePreference::Latex => {
                match self.resolve_engine_program(
                    pandoc::DEFAULT_ENGINE_LATEX,
                    LatexEngineOrigin::Path,
                ) {
                    Ok(engine) => Ok(engine),
                    Err(xelatex_error) => self.resolve_tectonic().map_err(|tectonic_error| {
                        OmniDocError::Other(format!(
                            "No usable native LaTeX engine was found. XeLaTeX: {xelatex_error} Tectonic: {tectonic_error}"
                        ))
                    }),
                }
            }
        }
    }

    fn resolve_tectonic(&self) -> Result<ResolvedLatexEngine> {
        if let Some(Some(configured)) = self.tool_paths.get("tectonic") {
            let mut engine = self
                .resolve_engine_program(configured, LatexEngineOrigin::Configured)
                .map_err(|_| {
                    OmniDocError::Other(format!(
                        "Configured Tectonic executable '{}' not found. Update the tectonic setting or install Tectonic.",
                        configured
                    ))
                })?;
            verify_tectonic_program(Path::new(&engine.executable)).map_err(|error| {
                OmniDocError::Other(format!(
                    "Configured Tectonic executable '{}' is unusable: {error}",
                    configured
                ))
            })?;
            engine.kind = LatexEngineKind::Tectonic;
            return Ok(engine);
        }

        let mut bundled_errors = Vec::new();
        for candidate in bundled_tectonic_candidates() {
            if candidate.is_file() {
                match verify_tectonic_program(&candidate) {
                    Ok(()) => {
                        return Ok(ResolvedLatexEngine {
                            executable: candidate.to_string_lossy().to_string(),
                            kind: LatexEngineKind::Tectonic,
                            origin: LatexEngineOrigin::Bundled,
                        });
                    }
                    Err(error) => bundled_errors.push(error),
                }
            }
        }

        match self.resolve_engine_program("tectonic", LatexEngineOrigin::Path) {
            Ok(mut engine) => {
                verify_tectonic_program(Path::new(&engine.executable)).map_err(|error| {
                    OmniDocError::Other(format!("Tectonic from PATH is unusable: {error}"))
                })?;
                engine.kind = LatexEngineKind::Tectonic;
                Ok(engine)
            }
            Err(_) if !bundled_errors.is_empty() => Err(OmniDocError::Other(format!(
                "Bundled Tectonic could not run ({}), and no usable Tectonic was found in PATH.",
                bundled_errors.join("; ")
            ))),
            Err(_) => Err(OmniDocError::Other(
                "Tectonic was not found in the OmniDoc package or PATH.".to_string(),
            )),
        }
    }

    fn resolve_engine_program(
        &self,
        program: &str,
        origin: LatexEngineOrigin,
    ) -> Result<ResolvedLatexEngine> {
        let path = PathBuf::from(program);
        let resolved = if path.is_file() {
            path
        } else {
            which::which(program).map_err(|_| {
                OmniDocError::Other(format!(
                    "Configured LaTeX engine '{}' not found. Install it or update the latex_engine setting.",
                    program
                ))
            })?
        };
        let executable = resolved.to_string_lossy().to_string();
        Ok(ResolvedLatexEngine {
            kind: classify_latex_engine(&resolved),
            executable,
            origin,
        })
    }

    /// 执行命令
    pub fn execute(&self, cmd: &str, args: &[&str], verbose: bool) -> Result<()> {
        self.execute_in_dir(cmd, args, verbose, None)
    }

    pub fn execute_in_dir(
        &self,
        cmd: &str,
        args: &[&str],
        verbose: bool,
        working_dir: Option<&Path>,
    ) -> Result<()> {
        self.execute_in_dir_with_env(cmd, args, verbose, working_dir, &[])
    }

    pub fn execute_in_dir_with_env(
        &self,
        cmd: &str,
        args: &[&str],
        verbose: bool,
        working_dir: Option<&Path>,
        environment: &[(OsString, OsString)],
    ) -> Result<()> {
        let tool_path = self.check_tool(cmd)?;

        self.execute_program_in_dir_with_env(&tool_path, args, verbose, working_dir, environment)
    }

    pub fn execute_program(&self, program: &str, args: &[&str], verbose: bool) -> Result<()> {
        self.execute_program_in_dir_with_env(program, args, verbose, None, &[])
    }

    pub fn execute_program_in_dir(
        &self,
        program: &str,
        args: &[&str],
        verbose: bool,
        working_dir: Option<&Path>,
    ) -> Result<()> {
        self.execute_program_in_dir_with_env(program, args, verbose, working_dir, &[])
    }

    pub fn execute_program_in_dir_with_env(
        &self,
        program: &str,
        args: &[&str],
        verbose: bool,
        working_dir: Option<&Path>,
        environment: &[(OsString, OsString)],
    ) -> Result<()> {
        let program = program.to_string();

        let mut command = Command::new(&program);
        command.args(args);
        command.envs(environment.iter().map(|(key, value)| (key, value)));
        if let Some(working_dir) = working_dir {
            command.current_dir(working_dir);
        }

        if verbose {
            println!("Executing: {} {}", program, args.join(" "));
        }

        let output = command.output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", program, e))
        })?;

        if !output.status.success() {
            let command = format!("{} {}", program, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());

            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command,
                output: diagnostic,
            });
        }

        if verbose {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                print!("{}", stdout);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                eprint!("{}", stderr);
            }
        }

        Ok(())
    }

    /// 执行命令并返回输出
    pub fn execute_with_output(&self, cmd: &str, args: &[&str]) -> Result<String> {
        let tool_path = self.check_tool(cmd)?;

        let output = Command::new(&tool_path).args(args).output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        if !output.status.success() {
            let command = format!("{} {}", tool_path, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());

            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command,
                output: diagnostic,
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// 异步执行命令（不等待完成）
    /// 用于启动后台进程或打开文件等场景
    pub fn spawn(&self, cmd: &str, args: &[&str]) -> Result<()> {
        let tool_path = self.check_tool(cmd)?;

        Command::new(&tool_path).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;

        Ok(())
    }

    /// 执行命令（不检查工具路径，直接使用命令名）
    /// 用于执行系统命令（如 make, xdg-open）等不需要检查工具的场景
    pub fn execute_system_cmd(&self, cmd: &str, args: &[&str], verbose: bool) -> Result<()> {
        let mut command = Command::new(cmd);
        command.args(args);

        if verbose {
            println!("Executing: {} {}", cmd, args.join(" "));
        }

        let output = command.output().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to execute '{}': {}", cmd, e))
        })?;

        // 输出 stdout 和 stderr
        std::io::stdout()
            .write_all(&output.stdout)
            .map_err(OmniDocError::Io)?;
        std::io::stderr()
            .write_all(&output.stderr)
            .map_err(OmniDocError::Io)?;

        if !output.status.success() {
            let command_str = format!("{} {}", cmd, args.join(" "));
            let diagnostic = summarize_command_output(&output.stdout, &output.stderr)
                .unwrap_or_else(|| "No command output was captured.".to_string());
            return Err(OmniDocError::CommandFailed {
                code: output.status.code(),
                command: command_str,
                output: diagnostic,
            });
        }

        Ok(())
    }

    /// 异步执行系统命令（不检查工具路径，直接使用命令名）
    /// 用于启动后台进程或打开文件等场景
    pub fn spawn_system_cmd(&self, cmd: &str, args: &[&str]) -> Result<()> {
        Command::new(cmd).args(args).spawn().map_err(|e| {
            OmniDocError::CommandExecution(format!("Failed to spawn '{}': {}", cmd, e))
        })?;

        Ok(())
    }
}

fn bundled_tectonic_candidates() -> Vec<PathBuf> {
    let Ok(executable) = std::env::current_exe() else {
        return Vec::new();
    };
    let Some(directory) = executable.parent() else {
        return Vec::new();
    };
    let binary_name = if cfg!(windows) {
        "tectonic.exe"
    } else {
        "tectonic"
    };
    let mut candidates = vec![
        directory.join(binary_name),
        directory.join("engines").join(binary_name),
    ];
    if let Some(prefix) = directory.parent() {
        candidates.push(prefix.join("lib").join("omnidoc").join(binary_name));
    }
    candidates
}

fn classify_latex_engine(path: &Path) -> LatexEngineKind {
    let stem = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match stem.as_str() {
        "tectonic" => LatexEngineKind::Tectonic,
        "xelatex" => LatexEngineKind::XeLatex,
        "lualatex" => LatexEngineKind::LuaLatex,
        "pdflatex" => LatexEngineKind::PdfLatex,
        _ => {
            let output = Command::new(path).arg("--version").output().ok();
            let version = output
                .as_ref()
                .map(|output| {
                    format!(
                        "{}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    )
                })
                .unwrap_or_default();
            if version.to_ascii_lowercase().contains("tectonic") {
                LatexEngineKind::Tectonic
            } else {
                LatexEngineKind::Other
            }
        }
    }
}

fn verify_tectonic_program(path: &Path) -> std::result::Result<(), String> {
    let output = Command::new(path)
        .arg("--version")
        .output()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        let detail = version.trim();
        return Err(if detail.is_empty() {
            format!("{} exited with {}", path.display(), output.status)
        } else {
            format!("{}: {detail}", path.display())
        });
    }
    if !version.to_ascii_lowercase().contains("tectonic") {
        return Err(format!(
            "{} did not identify itself as Tectonic",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{verify_tectonic_program, BuildExecutor, LatexEnginePreference};
    use std::collections::HashMap;

    #[test]
    fn configured_missing_latex_engine_does_not_fallback() {
        let mut tool_paths = HashMap::new();
        tool_paths.insert(
            "latex_engine".to_string(),
            Some("__omnidoc_missing_latex_engine__".to_string()),
        );
        let executor = BuildExecutor::new(tool_paths);

        let err = executor
            .check_tool("latex_engine")
            .expect_err("missing configured engine should fail");

        assert!(err.to_string().contains("__omnidoc_missing_latex_engine__"));
    }

    #[test]
    fn configured_missing_tool_does_not_fallback() {
        let mut tool_paths = HashMap::new();
        tool_paths.insert(
            "pandoc".to_string(),
            Some("__omnidoc_missing_pandoc__".to_string()),
        );
        let executor = BuildExecutor::new(tool_paths);

        let err = executor
            .check_tool("pandoc")
            .expect_err("missing configured tool should fail");

        assert!(err.to_string().contains("__omnidoc_missing_pandoc__"));
    }

    #[test]
    fn configured_missing_tectonic_does_not_fallback_for_markdown() {
        let mut tool_paths = HashMap::new();
        tool_paths.insert(
            "tectonic".to_string(),
            Some("__omnidoc_missing_tectonic__".to_string()),
        );
        let executor = BuildExecutor::new(tool_paths);

        let err = executor
            .resolve_latex_engine(LatexEnginePreference::Markdown)
            .expect_err("missing configured Tectonic should fail");

        assert!(err.to_string().contains("__omnidoc_missing_tectonic__"));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_non_tectonic_program_with_a_tectonic_role() {
        let err = verify_tectonic_program(std::path::Path::new("/bin/true"))
            .expect_err("an unrelated executable must not be accepted");

        assert!(err.contains("did not identify itself as Tectonic"));
    }

    #[cfg(unix)]
    #[test]
    fn configured_tectonic_role_wins_over_a_misleading_file_name() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("fake Tectonic root");
        let engine = root.path().join("xelatex");
        fs::write(&engine, "#!/bin/sh\nprintf 'Tectonic 0.16.9\\n'\n")
            .expect("fake Tectonic executable");
        let mut permissions = fs::metadata(&engine)
            .expect("engine metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&engine, permissions).expect("engine permissions");
        let executor = BuildExecutor::new(HashMap::from([(
            "tectonic".to_string(),
            Some(engine.to_string_lossy().to_string()),
        )]));

        let resolved = executor
            .resolve_latex_engine(LatexEnginePreference::Markdown)
            .expect("configured Tectonic");

        assert!(resolved.is_tectonic());
    }

    #[cfg(unix)]
    #[test]
    fn executes_commands_in_the_requested_working_directory() {
        let directory = tempfile::tempdir().expect("working directory");
        let executor = BuildExecutor::new(HashMap::new());
        let expected = directory
            .path()
            .canonicalize()
            .expect("canonical working directory")
            .to_string_lossy()
            .to_string();

        executor
            .execute_in_dir(
                "sh",
                &["-c", "test \"$(pwd -P)\" = \"$1\"", "sh", &expected],
                false,
                Some(directory.path()),
            )
            .expect("command should see requested working directory");
    }
}
