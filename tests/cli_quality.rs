use sha2::{Digest, Sha256};
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
        let home = env_root.join("home");
        fs::create_dir_all(project.join("build")).expect("project build dir");
        fs::create_dir_all(project.join("plugins").join("sample")).expect("plugin dir");
        fs::create_dir_all(&home).expect("fake home dir");
        fs::create_dir_all(
            home.join("Library")
                .join("Application Support")
                .join("omnidoc"),
        )
        .expect("fake macOS app support dir");
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
            r#"manifest_version = 1
key = "sample"
name = "Sample Plugin"
version = "0.1.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
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
        self.command_builder(args).output().expect("run omnidoc")
    }

    fn command_in(&self, current_dir: &Path, args: &[&str]) -> Output {
        let mut command = self.command_builder(args);
        command.current_dir(current_dir);
        command.output().expect("run omnidoc")
    }

    fn command_in_with_env(
        &self,
        current_dir: &Path,
        args: &[&str],
        env: &[(&str, &Path)],
    ) -> Output {
        let mut command = self.command_builder(args);
        command.current_dir(current_dir);
        for (name, value) in env {
            command.env(name, value);
        }
        command.output().expect("run omnidoc")
    }

    fn command_builder(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_omnidoc"));
        command
            .args(args)
            .env("HOME", self.env_root.join("home"))
            .env("XDG_CONFIG_HOME", self.env_root.join("config"))
            .env("XDG_DATA_HOME", self.env_root.join("data"));
        command
    }

    fn project_arg(&self) -> String {
        self.project.display().to_string()
    }

    fn base(&self) -> &Path {
        self.project.parent().expect("fixture base")
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
fn help_prioritizes_workflows_and_keeps_legacy_commands_out_of_the_main_list() {
    let fixture = Fixture::new("help-groups");
    let output = fixture.command(&["--help"]);
    let stdout = assert_success(output);

    assert!(stdout.contains("omnidoc check --help"));
    assert!(stdout.contains("omnidoc convert --help"));
    assert!(stdout.contains("omnidoc template --help"));
    assert!(!stdout.contains("  config-validate  "));
    assert!(!stdout.contains("  md2pdf  "));

    let new_help = assert_success(fixture.command(&["new", "--help"]));
    assert!(new_help.contains("Usage: omnidoc new [OPTIONS] <PATH>"));
    assert!(new_help.contains("--type <KEY>"));
    assert!(new_help.contains("--defaults"));
    assert!(!new_help.contains("--title <TITLE> <PATH>"));

    let config_help = assert_success(fixture.command(&["config", "--help"]));
    assert!(config_help.contains("  init  "));
    assert!(config_help.contains("  show  "));
    assert!(config_help.contains("  get   "));
    assert!(!config_help.contains("-l, --lib <LIB>"));
    assert!(!config_help.contains("-o, --outdir <OUTDIR>"));
}

#[test]
fn new_supports_non_interactive_templates_and_infers_the_title() {
    let fixture = Fixture::new("new-direct");
    let target = fixture.base().join("my-guide");

    let stdout = assert_success(fixture.command_in(
        fixture.base(),
        &["new", "my-guide", "--type", "ctex-md", "--author", "Tester"],
    ));

    assert!(stdout.contains("Next:"));
    assert!(target.join(".omnidoc.toml").is_file());
    let main = fs::read_to_string(target.join("main.md")).expect("generated entry");
    assert!(main.contains("title: my guide"));
    assert!(main.contains("- Tester"));
    let config = fs::read_to_string(target.join(".omnidoc.toml")).expect("project config");
    assert!(config.contains("entry = \"main.md\""));
    assert!(config.contains("from = \"markdown\""));

    let repo = git2::Repository::open(&target).expect("created git repository");
    let commit = repo
        .head()
        .expect("repository head")
        .peel_to_commit()
        .expect("initial commit");
    let tree = commit.tree().expect("initial tree");
    assert!(tree.get_path(Path::new(".omnidoc.toml")).is_ok());
}

#[test]
fn non_interactive_new_without_a_template_fails_before_creating_the_path() {
    let fixture = Fixture::new("new-no-template");
    let target = fixture.base().join("needs-choice");
    let output = fixture.command_in(fixture.base(), &["new", "needs-choice"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--type <KEY>"));
    assert!(!target.exists());

    let invalid_target = fixture.base().join("invalid-template");
    let invalid = fixture.command_in(
        fixture.base(),
        &["new", "invalid-template", "--type", "ctex-m"],
    );
    assert!(!invalid.status.success());
    let stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(stderr.contains("did you mean 'ctex-md'"));
    assert!(stderr.contains("omnidoc template list"));
    assert!(!invalid_target.exists());
}

#[test]
fn init_accepts_an_existing_main_file_and_resolves_relative_paths_once() {
    let fixture = Fixture::new("init-relative");
    let target = fixture.base().join("existing-notes");
    fs::create_dir_all(&target).expect("existing repository");
    fs::write(target.join("main.md"), "# Existing notes\n").expect("existing entry");

    assert_success(fixture.command_in(
        fixture.base(),
        &[
            "init",
            "existing-notes",
            "--type",
            "ctex-md",
            "--title",
            "Existing Notes",
        ],
    ));

    assert!(target.join(".omnidoc.toml").is_file());
    assert!(!target.join("existing-notes/.omnidoc.toml").exists());
    assert_eq!(
        fs::read_to_string(target.join("main.md")).expect("preserved entry"),
        "# Existing notes\n"
    );
}

#[test]
fn relative_update_resolves_the_target_and_infers_the_project_format() {
    let fixture = Fixture::new("update-relative");

    assert_success(fixture.command_in(fixture.base(), &["update", "project"]));

    assert!(fixture.project.join(".gitignore").is_file());
    assert!(!fixture.project.join(".latexmkrc").exists());
    assert!(!fixture.project.join("project/.gitignore").exists());

    let repository = git2::Repository::open(&fixture.project).expect("updated repository");
    let commit = repository
        .head()
        .expect("repository head")
        .peel_to_commit()
        .expect("update commit");
    assert_eq!(commit.message().expect("commit message"), "Update project");
}

#[test]
fn update_preview_is_json_and_does_not_modify_the_project() {
    let fixture = Fixture::new("update-preview");
    fs::write(fixture.project.join("notes.md"), "# Notes\n").expect("root markdown");
    fs::write(fixture.project.join("appendix.tex"), "Appendix\n").expect("root latex");

    let output =
        assert_success(fixture.command(&["update", "--dry-run", "--json", &fixture.project_arg()]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("update preview JSON");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["commit"], true);
    assert_eq!(report["applied"], false);

    let operations = report["actions"]
        .as_array()
        .expect("update actions")
        .iter()
        .filter_map(|action| action["operation"].as_str())
        .collect::<Vec<_>>();
    for expected in [
        "refresh_file",
        "initialize_git",
        "create_directory",
        "move_file",
        "commit",
    ] {
        assert!(operations.contains(&expected), "missing action {expected}");
    }
    assert!(
        report["actions"].as_array().is_some_and(|actions| {
            actions.iter().any(|action| {
                action["operation"] == "move_file"
                    && action["path"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("/appendix.tex"))
                    && action["destination"]
                        .as_str()
                        .is_some_and(|path| path.ends_with("/tex/appendix.tex"))
            })
        }),
        "report: {report}"
    );

    assert!(fixture.project.join("notes.md").is_file());
    assert!(fixture.project.join("appendix.tex").is_file());
    assert!(!fixture.project.join("md/notes.md").exists());
    assert!(!fixture.project.join("tex/appendix.tex").exists());
    assert!(!fixture.project.join(".git").exists());
    assert!(!fixture.project.join(".gitignore").exists());
    assert!(!fixture.project.join(".omnidoc-cache").exists());
}

#[test]
fn update_no_commit_moves_mixed_sources_and_can_be_committed_later() {
    let fixture = Fixture::new("update-no-commit");
    fs::write(fixture.project.join("notes.md"), "# Notes\n").expect("root markdown");
    fs::write(fixture.project.join("appendix.tex"), "Appendix\n").expect("root latex");

    let output = assert_success(fixture.command(&[
        "update",
        "--no-commit",
        "--json",
        &fixture.project_arg(),
    ]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("update report JSON");
    assert_eq!(report["dry_run"], false);
    assert_eq!(report["commit"], false);
    assert_eq!(report["applied"], true);
    assert!(report["actions"]
        .as_array()
        .is_some_and(|actions| { actions.iter().all(|action| action["operation"] != "commit") }));

    assert!(fixture.project.join(".git").is_dir());
    assert!(fixture.project.join(".gitignore").is_file());
    assert!(fixture.project.join("md/notes.md").is_file());
    assert!(fixture.project.join("tex/appendix.tex").is_file());
    assert!(!fixture.project.join("notes.md").exists());
    assert!(!fixture.project.join("appendix.tex").exists());

    let repository = git2::Repository::open(&fixture.project).expect("uncommitted repository");
    assert!(repository.head().is_err());

    assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let repository = git2::Repository::open(&fixture.project).expect("committed repository");
    let commit = repository
        .head()
        .expect("repository head")
        .peel_to_commit()
        .expect("first update commit");
    assert_eq!(commit.message().expect("commit message"), "Update project");
}

#[test]
fn update_rejects_source_move_collisions_before_writing() {
    let fixture = Fixture::new("update-collision");
    fs::create_dir_all(fixture.project.join("md")).expect("markdown directory");
    fs::write(fixture.project.join("notes.md"), "root copy\n").expect("root markdown");
    fs::write(fixture.project.join("md/notes.md"), "existing copy\n").expect("existing markdown");

    let output = fixture.command(&["update", "--json", &fixture.project_arg()]);
    let stdout = assert_failure(output);
    let error: serde_json::Value = serde_json::from_str(&stdout).expect("update error JSON");
    assert_eq!(error["error"]["category"], "project");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("destination already exists")));

    assert_eq!(
        fs::read_to_string(fixture.project.join("notes.md")).expect("root source"),
        "root copy\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.project.join("md/notes.md")).expect("existing source"),
        "existing copy\n"
    );
    assert!(!fixture.project.join(".git").exists());
    assert!(!fixture.project.join(".gitignore").exists());
    assert!(!fixture.project.join(".omnidoc-cache").exists());
}

#[test]
fn external_templates_work_for_direct_creation_listing_and_validation() {
    let fixture = Fixture::new("external-template");
    let templates = fixture.base().join("templates");
    let custom = templates.join("team-note");
    fs::create_dir_all(&custom).expect("external template directory");
    fs::write(
        custom.join("manifest.toml"),
        r#"key = "team-note"
name = "Team Note"
description = "A short team note"
language = "markdown"
template_file = "template.md"
file_name = "docs/index.md"
"#,
    )
    .expect("external template manifest");
    fs::write(
        custom.join("template.md"),
        "# {{ title }}\n\nOwner: {{ author }}\n",
    )
    .expect("external template body");

    let env = [("OMNIDOC_TEMPLATE_DIR", templates.as_path())];
    assert_success(fixture.command_in_with_env(
        fixture.base(),
        &[
            "new",
            "team-handbook",
            "--type",
            "team-note",
            "--author",
            "Docs Team",
        ],
        &env,
    ));

    let target = fixture.base().join("team-handbook");
    let entry = fs::read_to_string(target.join("docs/index.md")).expect("external entry");
    assert!(entry.contains("# team handbook"));
    assert!(entry.contains("Owner: Docs Team"));
    let config = fs::read_to_string(target.join(".omnidoc.toml")).expect("project config");
    assert!(config.contains("entry = \"docs/index.md\""));

    let listed = assert_success(fixture.command_in_with_env(
        fixture.base(),
        &["template", "list", "--json"],
        &env,
    ));
    let listed: serde_json::Value = serde_json::from_str(&listed).expect("template list JSON");
    assert!(listed.as_array().is_some_and(|templates| templates
        .iter()
        .any(|template| template["key"] == "team-note")));

    let validated = assert_success(fixture.command_in_with_env(
        fixture.base(),
        &["template", "validate", "team-note", "--json"],
        &env,
    ));
    let validated: serde_json::Value =
        serde_json::from_str(&validated).expect("template validation JSON");
    assert_eq!(validated[0]["valid"], true);

    let unsafe_template = templates.join("unsafe-note");
    fs::create_dir_all(&unsafe_template).expect("unsafe template directory");
    fs::write(
        unsafe_template.join("manifest.toml"),
        r#"key = "unsafe-note"
language = "markdown"
template_file = "template.md"
file_name = "../../escaped.md"
"#,
    )
    .expect("unsafe template manifest");
    fs::write(unsafe_template.join("template.md"), "# {{ title }}\n")
        .expect("unsafe template body");

    let invalid = assert_failure(fixture.command_in_with_env(
        fixture.base(),
        &["template", "validate", "unsafe-note", "--json"],
        &env,
    ));
    let invalid: serde_json::Value =
        serde_json::from_str(&invalid).expect("invalid template validation JSON");
    assert_eq!(invalid[0]["valid"], false);
    assert!(invalid[0]["error"]
        .as_str()
        .is_some_and(|error| error.contains("safe relative path")));

    assert_failure(fixture.command_in_with_env(
        fixture.base(),
        &["new", "unsafe-project", "--type", "unsafe-note"],
        &env,
    ));
    assert!(!fixture.base().join("unsafe-project").exists());
    assert!(!fixture.base().join("escaped.md").exists());
}

#[test]
fn fatal_errors_use_the_structured_terminal_layout() {
    let fixture = Fixture::new("error-layout");
    fs::write(
        fixture.project.join(".omnidoc.toml"),
        "[project]\nentry = \"missing.md\"\nfrom = \"markdown\"\nto = \"html\"\n",
    )
    .expect("invalid project config");

    let output = fixture.command(&["config-validate", &fixture.project_arg()]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(stderr.contains("error: configuration validation failed\n"));
    assert!(stderr.contains("  context: configuration\n"));
    assert!(!stderr.contains('✖'));
    assert!(!stderr.contains("Configuration error:"));
}

#[test]
fn quality_commands_work_on_minimal_project() {
    let fixture = Fixture::new("quality");
    let project = fixture.project_arg();

    assert_success(fixture.command(&["config-validate", &project]));
    assert_success(fixture.command(&["lint", "--strict", &project]));
    assert_success(fixture.command(&["check", "config", &project]));
    assert_success(fixture.command(&["check", "lint", "--strict", &project]));

    let deps = assert_success(fixture.command(&["deps", "--json", &project]));
    assert!(deps.contains("main.md"));
    assert!(deps.contains(".omnidoc.toml"));
    let grouped_deps = assert_success(fixture.command(&["check", "deps", "--json", &project]));
    assert!(grouped_deps.contains("main.md"));

    assert_success(fixture.command(&["lock", "--update", &project]));
    assert_success(fixture.command(&["lock", "--check", &project]));

    let plugins = assert_success(fixture.command(&["plugin", "--json", "--validate", &project]));
    assert!(plugins.contains("sample"));
    assert!(plugins.contains("\"valid\": true"));
}

#[test]
fn project_quality_and_publish_commands_resolve_nested_invocations() {
    let fixture = Fixture::new("nested-project-commands");
    let nested = fixture.project.join("chapters/drafts");
    fs::create_dir_all(&nested).expect("nested directory");

    assert_success(fixture.command_in(&nested, &["check", "config"]));
    assert_success(fixture.command_in(&nested, &["check", "lint", "--strict"]));

    let deps = assert_success(fixture.command_in(&nested, &["check", "deps", "--json"]));
    assert!(deps.contains("main.md"));
    assert!(deps.contains(".omnidoc.toml"));

    assert_success(fixture.command_in(&nested, &["check", "lock", "--update"]));
    assert!(fixture.project.join("omnidoc.lock").is_file());
    assert!(!nested.join("omnidoc.lock").exists());

    let plugins = assert_success(fixture.command_in(&nested, &["plugin", "--json", "--validate"]));
    assert!(plugins.contains("sample"));

    assert_success(fixture.command_in(
        &nested,
        &[
            "publish",
            "--to",
            "html",
            "--no-build",
            "--tag",
            "nested-release",
        ],
    ));
    assert!(fixture
        .project
        .join("dist/nested-release/smoke.html")
        .is_file());
    assert!(!nested.join("dist").exists());
}

#[test]
fn update_runs_at_the_project_root_when_invoked_from_a_nested_directory() {
    let fixture = Fixture::new("nested-update");
    let nested = fixture.project.join("chapters/drafts");
    fs::create_dir_all(&nested).expect("nested directory");

    assert_success(fixture.command_in(&nested, &["update"]));

    assert!(fixture.project.join(".gitignore").is_file());
    assert!(!nested.join(".gitignore").exists());
    assert!(git2::Repository::open(&fixture.project).is_ok());
}

#[test]
fn status_and_open_resolve_the_configured_artifact_contract() {
    let fixture = Fixture::new("project-status");
    let project = fixture.project_arg();
    let nested = fixture.project.join("chapters/drafts");
    fs::create_dir_all(&nested).expect("nested project directory");

    let output = assert_success(fixture.command_in(&nested, &["status", "--json"]));
    let status: serde_json::Value = serde_json::from_str(&output).expect("project status JSON");
    assert_eq!(status["schema_version"], 1);
    assert_eq!(
        Path::new(status["project_root"].as_str().expect("project root")),
        fixture.project
    );
    assert_eq!(status["source_format"], "markdown");
    assert_eq!(status["target"], "smoke");
    assert_eq!(status["default_output"], "html");
    assert_eq!(status["configured_outputs"], serde_json::json!(["html"]));
    assert_eq!(status["entry"]["exists"], true);
    assert_eq!(status["artifacts"][0]["format"], "html");
    assert_eq!(status["artifacts"][0]["exists"], true);
    assert!(status["artifacts"][0]["path"]
        .as_str()
        .is_some_and(|path| path.ends_with("/build/smoke.html")));

    let artifact =
        assert_success(fixture.command_in(&nested, &["open", "--to", "html", "--print-path"]));
    assert_eq!(
        Path::new(artifact.trim()),
        fixture.project.join("build/smoke.html")
    );

    let missing = fixture.command(&["open", "--to", "pdf", "--print-path", &project]);
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("omnidoc build --to pdf"));
}

#[test]
fn clean_preview_is_json_and_does_not_modify_the_project() {
    let fixture = Fixture::new("clean-preview");
    let project = fixture.project_arg();
    fs::write(fixture.project.join("reference.pdf"), "source asset\n").expect("source PDF");

    let output = assert_success(fixture.command(&["clean", "--dry-run", "--json", &project]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("clean preview JSON");
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["mode"], "clean");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["removed_targets"], 0);
    assert_eq!(report["targets"][0]["kind"], "directory");
    assert_eq!(report["targets"][0]["files"], 1);
    assert!(fixture.project.join("build/smoke.html").is_file());
    assert!(fixture.project.join("reference.pdf").is_file());
    assert!(!fixture.project.join(".omnidoc-cache").exists());

    let output = assert_success(fixture.command(&["clean", "--json", &project]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("clean report JSON");
    assert_eq!(report["removed_targets"], 1);
    assert!(!fixture.project.join("build").exists());
    assert!(fixture.project.join("reference.pdf").is_file());
}

#[test]
fn distclean_preserves_unrelated_pdfs_and_rejects_escaping_output_directories() {
    let fixture = Fixture::new("clean-safety");
    let project = fixture.project_arg();
    fs::write(fixture.project.join("reference.pdf"), "source asset\n").expect("source PDF");
    fs::write(fixture.project.join("smoke.aux"), "temporary\n").expect("temporary file");
    fs::create_dir_all(fixture.project.join("auto")).expect("auto dir");
    fs::write(fixture.project.join("auto/generated.el"), "generated\n").expect("auto file");

    assert_success(fixture.command(&["clean", "--distclean", &project]));
    assert!(!fixture.project.join("build").exists());
    assert!(!fixture.project.join("smoke.aux").exists());
    assert!(!fixture.project.join("auto").exists());
    assert!(fixture.project.join("reference.pdf").is_file());

    let shared = fixture.base().join("shared");
    fs::create_dir_all(&shared).expect("shared output dir");
    fs::write(shared.join("keep.txt"), "keep\n").expect("shared file");
    fs::write(
        fixture.project.join(".omnidoc.toml"),
        r#"[project]
entry = "main.md"
from = "markdown"
to = "html"
target = "smoke"

[build]
outdir = "../shared"
"#,
    )
    .expect("unsafe output config");

    let output = fixture.command(&["clean", "--dry-run", "--json", &project]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("escapes the project root"));
    let error: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("structured clean error");
    assert_eq!(error["error"]["category"], "project");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("escapes the project root")));
    assert!(shared.join("keep.txt").is_file());
}

#[test]
fn doctor_checks_only_the_configured_output_toolchain() {
    let fixture = Fixture::new("doctor-html");
    let project = fixture.project_arg();

    let output = assert_success(fixture.command(&["doctor", "--json", &project]));
    let checks: serde_json::Value = serde_json::from_str(&output).expect("doctor JSON");
    let checks = checks.as_array().expect("doctor check array");
    let names = checks
        .iter()
        .filter_map(|check| check["name"].as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"pandoc"));
    assert!(names.contains(&"pandoc-crossref"));
    assert!(names.contains(&"omnidoc-libs"));
    assert!(names.contains(&"config"));
    assert!(!names.contains(&"latex-engine"));
    assert!(!names.contains(&"latexmk"));
    assert!(!names.contains(&"epubcheck"));
    assert!(checks
        .iter()
        .all(|check| { check["ok"].is_boolean() && check["detail"].is_string() }));
}

#[test]
fn doctor_can_scope_checks_to_one_output() {
    let fixture = Fixture::new("doctor-output");
    fs::write(
        fixture.project.join(".omnidoc.toml"),
        r#"[project]
entry = "main.md"
from = "markdown"
to = "html"
target = "smoke"

[build]
outdir = "build"
outputs = ["html", "epub"]
"#,
    )
    .expect("multi-output config");

    let output = assert_success(fixture.command(&[
        "doctor",
        "--json",
        "--output",
        "html",
        &fixture.project_arg(),
    ]));
    let checks: serde_json::Value = serde_json::from_str(&output).expect("doctor JSON");
    let names = checks
        .as_array()
        .expect("doctor checks")
        .iter()
        .filter_map(|check| check["name"].as_str())
        .collect::<Vec<_>>();
    assert!(!names.contains(&"epubcheck"));
}

#[test]
fn doctor_reports_configured_missing_tools_and_themes() {
    let fixture = Fixture::new("doctor-missing");
    fs::write(
        fixture.env_root.join("config/omnidoc.toml"),
        "[tools]\npandoc = \"__omnidoc_missing_pandoc__\"\n",
    )
    .expect("global tool config");
    fs::write(
        fixture.project.join(".omnidoc.toml"),
        r#"[project]
entry = "main.md"
from = "markdown"
to = "html"
target = "smoke"

[build]
outdir = "build"
outputs = ["html"]

[theme]
name = "missing-theme"
"#,
    )
    .expect("themed project config");

    let output = assert_success(fixture.command(&["doctor", "--json", &fixture.project_arg()]));
    let checks: serde_json::Value = serde_json::from_str(&output).expect("doctor JSON");
    let checks = checks.as_array().expect("doctor check array");
    let pandoc = checks
        .iter()
        .find(|check| check["name"] == "pandoc")
        .expect("pandoc check");
    let theme = checks
        .iter()
        .find(|check| check["name"] == "theme:missing-theme")
        .expect("theme check");

    assert_eq!(pandoc["ok"], false);
    assert!(pandoc["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("__omnidoc_missing_pandoc__")));
    assert_eq!(theme["ok"], false);
    assert!(theme["detail"].as_str().is_some_and(|detail| {
        detail.contains("missing-theme")
            || detail.contains("theme manifest")
            || detail.contains("configured library")
    }));

    let strict = fixture.command(&["doctor", "--strict", "--json", &fixture.project_arg()]);
    let strict_json = assert_failure(strict);
    let strict_checks: serde_json::Value =
        serde_json::from_str(&strict_json).expect("strict doctor JSON");
    assert!(strict_checks
        .as_array()
        .is_some_and(|checks| checks.iter().any(|check| check["ok"] == false)));
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
fn library_status_and_verify_use_the_versioned_manifest() {
    let fixture = Fixture::new("library-verify");
    let library = fixture.env_root.join("library");
    fs::create_dir_all(library.join("payload")).expect("payload dir");
    let payload = b"verified payload\n";
    fs::write(library.join("payload/resource.txt"), payload).expect("payload");
    let checksum = format!("{:x}", Sha256::digest(payload));
    fs::write(
        library.join("manifest.toml"),
        r#"manifest_version = 1
version = "1.0.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
compatible_pandoc = ">=0.0.0"
checksum_algorithm = "sha256"
checksum_file = "checksums.sha256"
payload_roots = ["payload"]
required_resources = ["payload/resource.txt"]
"#,
    )
    .expect("manifest");
    fs::write(
        library.join("checksums.sha256"),
        format!("{}  payload/resource.txt\n", checksum),
    )
    .expect("checksums");
    fs::write(
        fixture.env_root.join("config/omnidoc.toml"),
        format!("[lib]\npath = {:?}\n", library.to_string_lossy()),
    )
    .expect("global config");

    let verified = assert_success(fixture.command(&["libs", "--verify", "--json"]));
    assert!(verified.contains("\"version\": \"1.0.0\""));
    assert!(verified.contains("\"integrity_verified\": true"));
    assert!(verified.contains("\"source\": \"local\""));

    fs::write(library.join("payload/resource.txt"), b"tampered\n").expect("tamper");
    let failed = assert_failure(fixture.command(&["lib", "--verify", "--json"]));
    assert!(failed.contains("\"integrity_verified\": false"));
    assert!(failed.contains("checksum mismatch"));
}

#[test]
fn theme_commands_discover_inspect_and_validate_bundles() {
    let fixture = Fixture::new("theme-bundle");
    let library = fixture.env_root.join("data/omnidoc");
    fs::create_dir_all(library.join("themes")).expect("theme manifests");
    fs::create_dir_all(library.join("pandoc/css")).expect("theme css");
    fs::create_dir_all(library.join("pandoc/data/filters")).expect("theme filters");
    fs::create_dir_all(library.join("texmf/tex/common")).expect("theme latex");
    fs::write(
        library.join("pandoc/css/engineering-book.css"),
        "body { max-width: 56rem; }\n",
    )
    .expect("css");
    fs::write(
        library.join("pandoc/data/filters/admonition.lua"),
        "return {}\n",
    )
    .expect("filter");
    fs::write(
        library.join("texmf/tex/common/omni-engineering-book.sty"),
        "% engineering book\n",
    )
    .expect("latex package");
    fs::write(
        library.join("themes/engineering-book.toml"),
        r#"manifest_version = 1
name = "engineering-book"
version = "1.0.0"
description = "Matching engineering book output styles"
compatible_omnidoc = ">=1.3.0,<2.0.0"
compatibility = "readium"

[resources]
html_css = ["pandoc/css/engineering-book.css"]
epub_css = ["pandoc/css/engineering-book.css"]
latex_packages = ["texmf/tex/common/omni-engineering-book.sty"]
lua_filters = ["pandoc/data/filters/admonition.lua"]

[requirements]
fonts = ["Noto Serif CJK SC"]

[metadata.defaults]
lang = "zh-CN"
"#,
    )
    .expect("theme manifest");
    fs::write(
        fixture.project.join(".omnidoc.toml"),
        r#"[project]
entry = "main.md"
from = "markdown"
to = "html"
target = "smoke"

[build]
outdir = "build"
outputs = ["html"]

[theme]
name = "engineering-book"
version = "1"
compatibility = "readium"
"#,
    )
    .expect("themed project config");

    let listed = assert_success(fixture.command(&["theme", "list", "--json"]));
    assert!(listed.contains("engineering-book"));
    assert!(listed.contains("\"valid\": true"));

    let inspected =
        assert_success(fixture.command(&["theme", "inspect", "engineering-book", "--json"]));
    assert!(inspected.contains("\"compatibility\": \"readium\""));
    assert!(inspected.contains("Noto Serif CJK SC"));
    assert_success(fixture.command(&["theme", "validate", "engineering-book"]));
    assert_success(fixture.command(&["config-validate", &fixture.project_arg()]));

    fs::remove_file(library.join("pandoc/css/engineering-book.css")).expect("remove css");
    let failed =
        assert_failure(fixture.command(&["theme", "validate", "engineering-book", "--json"]));
    assert!(failed.contains("missing theme resource"));
}

#[test]
fn read_only_json_commands_do_not_create_default_config() {
    let fixture = Fixture::new("json-default-config");
    fs::remove_file(fixture.env_root.join("config/omnidoc.toml")).ok();

    let output = fixture.command(&["theme", "list", "--json"]);
    let stdout = assert_success(output);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("pure JSON stdout");
    assert_eq!(parsed, serde_json::json!([]));
    assert!(!fixture.env_root.join("config/omnidoc.toml").exists());
}

#[test]
fn config_show_and_get_are_read_only_and_machine_readable() {
    let fixture = Fixture::new("config-read");
    let project = fixture.project_arg();

    let output = assert_success(fixture.command(&["config", "show", "--json", &project]));
    let shown: serde_json::Value = serde_json::from_str(&output).expect("config show JSON");
    assert_eq!(shown["schema_version"], 1);
    assert_eq!(shown["scope"], "merged");
    assert_eq!(shown["config"]["target"], "smoke");
    assert!(shown["sources"].as_array().is_some_and(|sources| {
        sources
            .iter()
            .any(|source| source["kind"] == "global" && source["exists"] == false)
            && sources
                .iter()
                .any(|source| source["kind"] == "project" && source["exists"] == true)
    }));

    let output = assert_success(fixture.command(&["config", "get", "target", "--json", &project]));
    let value: serde_json::Value = serde_json::from_str(&output).expect("config get JSON");
    assert_eq!(value["scope"], "merged");
    assert_eq!(value["key"], "target");
    assert_eq!(value["value"], "smoke");

    let project_config = assert_success(
        fixture.command(&["config", "show", "--scope", "project", "--json", &project]),
    );
    let project_config: serde_json::Value =
        serde_json::from_str(&project_config).expect("project config JSON");
    assert_eq!(project_config["config"]["project"]["target"], "smoke");

    let missing = fixture.command(&["config", "get", "missing.key", "--json", &project]);
    assert!(!missing.status.success());
    let error: serde_json::Value =
        serde_json::from_slice(&missing.stdout).expect("config error JSON");
    assert_eq!(error["error"]["category"], "configuration");
    assert!(!fixture.env_root.join("config/omnidoc.toml").exists());
}

#[test]
fn config_init_and_legacy_generation_forms_remain_supported() {
    let grouped = Fixture::new("config-init");
    assert_success(grouped.command(&[
        "config",
        "init",
        "--author",
        "Docs Team",
        "--outdir",
        "output",
    ]));
    let grouped_config =
        fs::read_to_string(grouped.env_root.join("config/omnidoc.toml")).expect("grouped config");
    assert!(grouped_config.contains("name = \"Docs Team\""));
    assert!(grouped_config.contains("outdir = \"output\""));
    assert!(grouped_config.contains("texinputs = \"./tex//:\""));
    assert!(grouped_config.contains("bibinputs = \"./biblio//:\""));
    assert!(grouped_config.contains(
        grouped
            .env_root
            .join("data/omnidoc")
            .to_string_lossy()
            .as_ref()
    ));

    let legacy = Fixture::new("config-legacy");
    assert_success(legacy.command(&["config", "--authors", "Legacy User"]));
    let legacy_config =
        fs::read_to_string(legacy.env_root.join("config/omnidoc.toml")).expect("legacy config");
    assert!(legacy_config.contains("name = \"Legacy User\""));
}

#[test]
fn formatter_is_conservative_and_idempotent_on_structured_markdown() {
    let fixture = Fixture::new("formatter");
    let markdown = fixture.project.join("structured.md");
    fs::write(
        &markdown,
        concat!(
            "---\n",
            "title: 中文ABC:原样\n",
            "---\n\n",
            "| 中文ABC, | value:raw |\n",
            "|---|---|\n\n",
            "```rust\n",
            "let value = \"中文ABC:raw\";\n",
            "```\n\n",
            "<widget-panel>\n",
            "<widget-panel>\n",
            "中文ABC:raw\n",
            "</widget-panel>\n",
            "结束后中文ABC:raw\n",
            "</widget-panel>\n\n",
            "\\newcommand{\\RawName}{中文ABC:raw}\n",
            ": 定义中文ABC:raw\n",
            "+------+-------+\n",
            "| 中文ABC:raw | value |\n",
            "+======+=======+\n\n",
            "正文中文ABC 与 ``code:中文ABC``。\n",
        ),
    )
    .expect("structured markdown");
    let path = markdown.to_string_lossy().to_string();

    assert_success(fixture.command(&["fmt", &path]));
    let once = fs::read(&markdown).expect("formatted markdown");
    let text = String::from_utf8_lossy(&once);
    assert!(text.contains("title: 中文ABC:原样"));
    assert!(text.contains("| 中文ABC, | value:raw |"));
    assert!(text.contains("let value = \"中文ABC:raw\";"));
    assert!(text.contains("中文ABC:raw\n</widget-panel>\n结束后中文ABC:raw"));
    assert!(text.contains("\\newcommand{\\RawName}{中文ABC:raw}"));
    assert!(text.contains(": 定义中文ABC:raw"));
    assert!(text.contains("+------+-------+\n| 中文ABC:raw | value |\n+======+=======+"));
    assert!(text.contains("正文中文 ABC"));
    assert!(text.contains("``code:中文ABC``"));

    assert_success(fixture.command(&["fmt", &path]));
    let twice = fs::read(&markdown).expect("formatted twice");
    assert_eq!(once, twice);

    assert_success(fixture.command(&["fmt", "--check", &path]));
    fs::write(&markdown, "正文中文ABC。\n").expect("unformatted markdown");
    let before_check = fs::read(&markdown).expect("source before check");
    let check = assert_failure(fixture.command(&["fmt", "--check", &path]));
    assert!(check.contains("would format"));
    assert_eq!(
        fs::read(&markdown).expect("source after check"),
        before_check
    );

    let diff = assert_failure(fixture.command(&["fmt", "--diff", &path]));
    assert!(diff.contains("--- a/"));
    assert!(diff.contains("+++ b/"));
    assert_eq!(
        fs::read(&markdown).expect("source after diff"),
        before_check
    );

    assert_success(fixture.command(&["fmt", &path]));
    assert_success(fixture.command(&["fmt", "--check", &path]));
}

#[test]
fn publish_no_build_copies_existing_artifacts() {
    let fixture = Fixture::new("publish");
    let project = fixture.project_arg();
    let publish_dir = fixture.project.join("dist").join("release-1");
    fs::create_dir_all(&publish_dir).expect("old publish directory");
    fs::write(publish_dir.join("stale.txt"), "stale\n").expect("stale artifact");

    assert_success(fixture.command(&[
        "publish",
        "--to",
        "html",
        "--no-build",
        "--tag",
        "release/1",
        &project,
    ]));

    assert!(Path::new(&publish_dir.join("smoke.html")).exists());
    assert!(!publish_dir.join("stale.txt").exists());
    let manifest = fs::read_to_string(publish_dir.join("omnidoc-publish.json")).expect("manifest");
    let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("publish JSON");
    assert_eq!(manifest["manifest_version"], 2);
    assert_eq!(manifest["tag"], "release/1");
    assert_eq!(
        manifest["library_contract"]["library"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let artifacts = manifest["artifacts"].as_array().expect("publish artifacts");
    assert!(artifacts.iter().any(|artifact| {
        artifact["destination"] == "smoke.html"
            && artifact["source"] == "build/smoke.html"
            && artifact["digest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("blake3:"))
    }));
    assert!(artifacts.iter().any(|artifact| {
        artifact["output"] == "library-contract" && artifact["destination"] == "omnidoc-libs.toml"
    }));
    assert!(publish_dir.join("omnidoc-libs.toml").is_file());

    let verified = assert_success(fixture.command(&[
        "publish",
        "--verify",
        "--json",
        "--tag",
        "release/1",
        &project,
    ]));
    let verified: serde_json::Value =
        serde_json::from_str(&verified).expect("publish verification JSON");
    assert_eq!(verified["valid"], true);

    fs::write(publish_dir.join("smoke.html"), "tampered\n").expect("tamper publish artifact");
    let failed = assert_failure(fixture.command(&[
        "publish",
        "--verify",
        "--json",
        "--tag",
        "release/1",
        &project,
    ]));
    let failed: serde_json::Value =
        serde_json::from_str(&failed).expect("failed verification JSON");
    assert_eq!(failed["valid"], false);
    assert!(failed["errors"]
        .as_array()
        .expect("verification errors")
        .iter()
        .any(|error| error
            .as_str()
            .is_some_and(|error| error.contains("digest mismatch"))));
}

#[test]
fn failed_publish_preserves_existing_release_directory() {
    let fixture = Fixture::new("publish-failure");
    let project = fixture.project_arg();
    let publish_dir = fixture.project.join("dist").join("stable");
    fs::create_dir_all(&publish_dir).expect("existing publish directory");
    fs::write(publish_dir.join("marker.txt"), "preserve\n").expect("release marker");

    assert_failure(fixture.command(&[
        "publish",
        "--to",
        "epub",
        "--no-build",
        "--tag",
        "stable",
        &project,
    ]));

    assert_eq!(
        fs::read_to_string(publish_dir.join("marker.txt")).expect("preserved marker"),
        "preserve\n"
    );
    assert!(!fs::read_dir(fixture.project.join("dist"))
        .expect("dist directory")
        .flatten()
        .any(|entry| entry.file_name().to_string_lossy().contains(".staging.")));
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
        r#"manifest_version = 1
key = "sample"
name = "Sample Plugin"
version = "0.1.0"
compatible_omnidoc = ">=1.3.0,<2.0.0"
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
