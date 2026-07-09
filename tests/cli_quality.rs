use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture {
    project: PathBuf,
    env_root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "omnidoc-it-{}-{}-{}",
            name,
            std::process::id(),
            suffix
        ));
        let project = base.join("project");
        let env_root = base.join("env");
        fs::create_dir_all(project.join("build")).expect("project build dir");
        fs::create_dir_all(project.join("plugins").join("sample")).expect("plugin dir");
        fs::create_dir_all(env_root.join("data").join("omnidoc")).expect("fake lib dir");
        fs::create_dir_all(env_root.join("config")).expect("config dir");

        fs::write(
            project.join(".omnidoc.toml"),
            r#"[project]
entry = "main.md"
from = "markdown"
to = "html"
target = "smoke"

[build]
outdir = "build"
outputs = ["html"]
"#,
        )
        .expect("project config");
        fs::write(project.join("main.md"), "# Smoke\n\nA small document.\n").expect("main md");
        fs::write(project.join("build").join("smoke.html"), "<h1>Smoke</h1>\n")
            .expect("html output");
        fs::write(
            project.join("plugins").join("sample").join("manifest.toml"),
            r#"key = "sample"
name = "Sample Plugin"
version = "0.1.0"
kind = "template"
language = "markdown"
template_file = "template.md"
"#,
        )
        .expect("plugin manifest");
        fs::write(
            project.join("plugins").join("sample").join("template.md"),
            "# {{ title }}\n",
        )
        .expect("plugin template");

        Self { project, env_root }
    }

    fn command(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omnidoc"));
        command
            .args(args)
            .env("HOME", self.env_root.join("home"))
            .env("XDG_CONFIG_HOME", self.env_root.join("config"))
            .env("XDG_DATA_HOME", self.env_root.join("data"));
        command.output().expect("run omnidoc")
    }

    fn project_arg(&self) -> String {
        self.project.display().to_string()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(base) = self.project.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }
}

fn assert_success(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    stdout
}

fn assert_failure(output: Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    stdout
}

#[test]
fn quality_commands_work_on_minimal_project() {
    let fixture = Fixture::new("quality");
    let project = fixture.project_arg();

    assert_success(fixture.command(&["config-validate", &project]));
    assert_success(fixture.command(&["lint", "--strict", &project]));

    let deps = assert_success(fixture.command(&["deps", "--json", &project]));
    assert!(deps.contains("main.md"));
    assert!(deps.contains(".omnidoc.toml"));

    assert_success(fixture.command(&["lock", "--update", &project]));
    assert_success(fixture.command(&["lock", "--check", &project]));

    let plugins = assert_success(fixture.command(&["plugin", "--json", "--validate", &project]));
    assert!(plugins.contains("sample"));
    assert!(plugins.contains("\"valid\": true"));
}

#[test]
fn lock_check_detects_stale_inputs() {
    let fixture = Fixture::new("lock-stale");
    let project = fixture.project_arg();

    assert_success(fixture.command(&["lock", "--update", &project]));
    fs::write(fixture.project.join("main.md"), "# Smoke\n\nChanged.\n").expect("change source");

    let stdout = assert_failure(fixture.command(&["lock", "--check", &project]));
    assert!(stdout.contains("\"up_to_date\": false"));
}

#[test]
fn publish_no_build_copies_existing_artifacts() {
    let fixture = Fixture::new("publish");
    let project = fixture.project_arg();

    assert_success(fixture.command(&[
        "publish",
        "--to",
        "html",
        "--no-build",
        "--tag",
        "release/1",
        &project,
    ]));

    let publish_dir = fixture.project.join("dist").join("release-1");
    assert!(Path::new(&publish_dir.join("smoke.html")).exists());
    let manifest = fs::read_to_string(publish_dir.join("omnidoc-publish.json")).expect("manifest");
    assert!(manifest.contains("\"tag\": \"release/1\""));
    assert!(manifest.contains("smoke.html"));
}

#[cfg(unix)]
#[test]
fn plugin_lint_rule_runs_from_cli() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("plugin-lint");
    let plugin_dir = fixture.project.join("plugins").join("sample");
    let lint_script = plugin_dir.join("lint.sh");
    fs::write(
        &lint_script,
        "#!/bin/sh\nprintf 'warning:main.md:2:1:plugin warning\\n'\n",
    )
    .expect("lint hook");
    let mut permissions = fs::metadata(&lint_script).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&lint_script, permissions).expect("permissions");
    fs::write(
        plugin_dir.join("manifest.toml"),
        r#"key = "sample"
name = "Sample Plugin"
version = "0.1.0"
kind = "template"
language = "markdown"
template_file = "template.md"

[hooks]
lint_rule = ["lint.sh"]
"#,
    )
    .expect("plugin manifest");

    let project = fixture.project_arg();
    let lint = assert_success(fixture.command(&["lint", &project]));
    assert!(lint.contains("Plugin sample: plugin warning"));

    let plugins = assert_success(fixture.command(&["plugin", "--json", "--validate", &project]));
    assert!(plugins.contains("\"lint_rule\""));
}
