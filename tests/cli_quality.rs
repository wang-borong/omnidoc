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
    assert!(stdout.contains("omnidoc plugin --help"));
    assert!(stdout.contains("omnidoc lib --help"));
    assert!(!stdout.contains("  config-validate  "));
    assert!(!stdout.contains("  md2pdf  "));

    let new_help = assert_success(fixture.command(&["new", "--help"]));
    assert!(new_help.contains("Usage: omnidoc new [OPTIONS] <PATH>"));
    assert!(new_help.contains("--type <KEY>"));
    assert!(new_help.contains("--defaults"));
    assert!(new_help.contains("--dry-run"));
    assert!(new_help.contains("--no-commit"));
    assert!(new_help.contains("--json"));
    assert!(!new_help.contains("--title <TITLE> <PATH>"));

    let init_help = assert_success(fixture.command(&["init", "--help"]));
    assert!(init_help.contains("--dry-run"));
    assert!(init_help.contains("--diff"));
    assert!(init_help.contains("--no-commit"));
    assert!(init_help.contains("--json"));

    let config_help = assert_success(fixture.command(&["config", "--help"]));
    assert!(config_help.contains("  init  "));
    assert!(config_help.contains("  show  "));
    assert!(config_help.contains("  get   "));
    assert!(config_help.contains("  set   "));
    assert!(config_help.contains("  unset "));
    assert!(!config_help.contains("-l, --lib <LIB>"));
    assert!(!config_help.contains("-o, --outdir <OUTDIR>"));

    let plugin_help = assert_success(fixture.command(&["plugin", "--help"]));
    assert!(plugin_help.contains("  list      "));
    assert!(plugin_help.contains("  validate  "));
    assert!(!plugin_help.contains("\n      --validate"));

    let library_help = assert_success(fixture.command(&["lib", "--help"]));
    assert!(library_help.contains("  install  "));
    assert!(library_help.contains("  update   "));
    assert!(library_help.contains("  status   "));
    assert!(library_help.contains("  verify   "));
    assert!(!library_help.contains("\n  -i, --install"));
}

#[test]
fn grouped_library_status_matches_the_legacy_form() {
    let fixture = Fixture::new("library-group");
    let grouped = assert_success(fixture.command(&["lib", "status", "--json"]));
    let legacy = assert_success(fixture.command(&["lib", "--status", "--json"]));
    let grouped: serde_json::Value = serde_json::from_str(&grouped).expect("grouped status JSON");
    let legacy: serde_json::Value = serde_json::from_str(&legacy).expect("legacy status JSON");

    assert_eq!(grouped, legacy);
    assert!(grouped["path"].is_string());
    assert!(grouped["errors"].is_array());
}

#[test]
fn grouped_plugin_json_errors_remain_machine_readable() {
    let fixture = Fixture::new("plugin-json-error");
    let missing = fixture.base().join("missing-project");
    let output = fixture.command(&["plugin", "list", &missing.display().to_string(), "--json"]);
    let stdout = assert_failure(output);
    let error: serde_json::Value =
        serde_json::from_str(&stdout).expect("structured plugin error JSON");

    assert_eq!(error["schema_version"], 1);
    assert!(error["error"]["category"].is_string());
    assert!(error["error"]["message"].is_string());
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
    assert_eq!(
        commit.message().expect("initial commit message"),
        "Create project"
    );
    assert_eq!(commit.parent_count(), 0);
    let tree = commit.tree().expect("initial tree");
    assert!(tree.get_path(Path::new(".omnidoc.toml")).is_ok());
    let mut revwalk = repo.revwalk().expect("project revision walk");
    revwalk.push_head().expect("walk project head");
    assert_eq!(revwalk.count(), 1, "new should create one initial commit");
}

#[test]
fn new_preview_and_json_creation_are_safe_and_composable() {
    let fixture = Fixture::new("new-preview");
    let preview_target = fixture.base().join("planned-guide");
    let preview = assert_success(fixture.command_in(
        fixture.base(),
        &[
            "new",
            "planned-guide",
            "--type",
            "ctexart-tex",
            "--author",
            "Preview Author",
            "--dry-run",
            "--json",
        ],
    ));
    let preview: serde_json::Value =
        serde_json::from_str(&preview).expect("new preview JSON report");
    assert_eq!(preview["schema_version"], 1);
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["commit"], true);
    assert_eq!(preview["title"], "planned guide");
    assert_eq!(preview["author"], "Preview Author");
    assert_eq!(preview["template"]["key"], "ctexart-tex");
    assert!(preview["actions"].as_array().is_some_and(|actions| {
        actions.iter().any(|action| action["operation"] == "commit")
            && actions.iter().any(|action| {
                action["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with("/.latexmkrc"))
            })
            && actions.iter().all(|action| {
                action["path"]
                    .as_str()
                    .is_none_or(|path| !path.contains("/./"))
            })
    }));
    assert!(!preview_target.exists());
    assert!(!fixture.env_root.join("config/omnidoc.toml").exists());

    let target = fixture.base().join("uncommitted-guide");
    let created = assert_success(fixture.command_in(
        fixture.base(),
        &[
            "new",
            "uncommitted-guide",
            "--type",
            "ctex-md",
            "--no-commit",
            "--json",
        ],
    ));
    let created: serde_json::Value =
        serde_json::from_str(&created).expect("new creation JSON report");
    assert_eq!(created["dry_run"], false);
    assert_eq!(created["applied"], true);
    assert_eq!(created["commit"], false);
    assert!(created["actions"]
        .as_array()
        .is_some_and(|actions| { actions.iter().all(|action| action["operation"] != "commit") }));
    assert!(target.join(".omnidoc.toml").is_file());
    assert!(target.join("main.md").is_file());
    let repository = git2::Repository::open(&target).expect("uncommitted project repository");
    assert!(repository.head().is_err());
}

#[test]
fn new_json_errors_suggest_init_without_touching_existing_paths() {
    let fixture = Fixture::new("new-existing-json");
    let target = fixture.base().join("existing-directory");
    fs::create_dir_all(&target).expect("existing target");
    fs::write(target.join("keep.txt"), "keep\n").expect("existing content");

    let output = fixture.command_in(
        fixture.base(),
        &["new", "existing-directory", "--type", "ctex-md", "--json"],
    );
    let stdout = assert_failure(output);
    let error: serde_json::Value =
        serde_json::from_str(&stdout).expect("structured new error JSON");
    assert_eq!(error["error"]["category"], "project");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("omnidoc init existing-directory")));
    assert_eq!(
        fs::read_to_string(target.join("keep.txt")).expect("preserved existing content"),
        "keep\n"
    );
    assert!(!target.join(".omnidoc.toml").exists());
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

    let json_target = fixture.base().join("json-needs-choice");
    let json = fixture.command_in(fixture.base(), &["new", "json-needs-choice", "--json"]);
    let stdout = assert_failure(json);
    let error: serde_json::Value =
        serde_json::from_str(&stdout).expect("structured template-choice error");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("--type <KEY>")));
    assert!(!json_target.exists());
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
fn init_preview_diff_and_json_application_are_safe_and_composable() {
    let fixture = Fixture::new("init-preview");
    let target = fixture.base().join("existing-guide");
    fs::create_dir_all(&target).expect("existing directory");
    fs::write(target.join("main.md"), "# Existing guide\n").expect("existing entry");
    fs::write(target.join("notes.md"), "# Notes\n").expect("existing notes");
    fs::write(target.join(".gitignore"), "custom-output/\n").expect("custom gitignore");

    let no_choice = fixture.command_in(fixture.base(), &["init", "existing-guide", "--json"]);
    let stdout = assert_failure(no_choice);
    let error: serde_json::Value =
        serde_json::from_str(&stdout).expect("structured init template-choice error");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("--type <KEY>")));
    assert!(!target.join(".omnidoc.toml").exists());

    let preview = assert_success(fixture.command_in(
        fixture.base(),
        &[
            "init",
            "existing-guide",
            "--type",
            "ctex-md",
            "--diff",
            "--json",
        ],
    ));
    let preview: serde_json::Value =
        serde_json::from_str(&preview).expect("init preview JSON report");
    assert_eq!(preview["schema_version"], 1);
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["diff"], true);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["ready"], true);
    assert_eq!(preview["repository"]["exists"], false);
    let actions = preview["actions"].as_array().expect("init actions");
    assert!(actions.iter().any(|action| {
        action["operation"] == "create_file"
            && action["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/.omnidoc.toml"))
    }));
    assert!(actions.iter().any(|action| {
        action["operation"] == "move_file"
            && action["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/notes.md"))
    }));
    assert!(actions.iter().any(|action| {
        action["operation"] == "refresh_file"
            && action["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/.gitignore"))
            && action["diff"]
                .as_str()
                .is_some_and(|diff| diff.contains("custom-output/"))
    }));
    assert!(!target.join(".omnidoc.toml").exists());
    assert!(!target.join(".git").exists());
    assert!(target.join("notes.md").is_file());
    assert_eq!(
        fs::read_to_string(target.join(".gitignore")).expect("unchanged gitignore"),
        "custom-output/\n"
    );

    let applied = assert_success(fixture.command_in(
        fixture.base(),
        &[
            "init",
            "existing-guide",
            "--type",
            "ctex-md",
            "--no-commit",
            "--json",
        ],
    ));
    let applied: serde_json::Value =
        serde_json::from_str(&applied).expect("init application JSON report");
    assert_eq!(applied["dry_run"], false);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["commit"], false);
    assert_eq!(applied["will_commit"], false);
    assert!(target.join(".omnidoc.toml").is_file());
    assert!(target.join("md/notes.md").is_file());
    assert!(!target.join("notes.md").exists());
    assert_eq!(
        fs::read_to_string(target.join("main.md")).expect("preserved main entry"),
        "# Existing guide\n"
    );
    let repository = git2::Repository::open(&target).expect("initialized repository");
    assert!(repository.head().is_err());
}

#[test]
fn init_json_reports_path_resolution_errors() {
    let fixture = Fixture::new("init-missing-json");
    let missing = fixture.base().join("missing-directory");
    let output = fixture.command(&[
        "init",
        &missing.display().to_string(),
        "--type",
        "ctex-md",
        "--json",
    ]);
    let stdout = assert_failure(output);
    let error: serde_json::Value =
        serde_json::from_str(&stdout).expect("structured init path error JSON");

    assert_eq!(error["schema_version"], 1);
    assert_eq!(error["error"]["category"], "project");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("does not exist")));
}

#[test]
fn init_refuses_to_commit_existing_changes_unless_no_commit_is_explicit() {
    let fixture = Fixture::new("init-dirty-repo");
    let target = fixture.base().join("existing-repo");
    fs::create_dir_all(&target).expect("existing repository");
    fs::write(target.join("main.md"), "# Existing\n").expect("existing entry");

    let repository = git2::Repository::init(&target).expect("initialize repository");
    let mut index = repository.index().expect("repository index");
    index
        .add_path(Path::new("main.md"))
        .expect("stage existing entry");
    let tree_id = index.write_tree().expect("write initial tree");
    let tree = repository.find_tree(tree_id).expect("initial tree");
    let signature =
        git2::Signature::now("OmniDoc Test", "omnidoc@example.invalid").expect("signature");
    let initial_commit = repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial files",
            &tree,
            &[],
        )
        .expect("initial commit");
    drop(tree);
    drop(repository);
    fs::write(target.join("notes.md"), "# Uncommitted notes\n").expect("uncommitted file");

    let preview = assert_success(fixture.command_in(
        fixture.base(),
        &[
            "init",
            "existing-repo",
            "--type",
            "ctex-md",
            "--dry-run",
            "--json",
        ],
    ));
    let preview: serde_json::Value =
        serde_json::from_str(&preview).expect("dirty init preview JSON");
    assert_eq!(preview["ready"], false);
    assert_eq!(preview["applied"], false);
    assert_eq!(preview["repository"]["clean"], false);
    assert!(preview["repository"]["changes"]
        .as_array()
        .is_some_and(|changes| changes.iter().any(|change| change["path"] == "notes.md")));
    assert!(!target.join(".omnidoc.toml").exists());

    let output = fixture.command_in(
        fixture.base(),
        &["init", "existing-repo", "--type", "ctex-md"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert_failure(output);
    assert!(stderr.contains("repository already has changes"));
    assert!(stderr.contains("--no-commit"));
    assert!(!target.join(".omnidoc.toml").exists());

    assert_success(fixture.command_in(
        fixture.base(),
        &["init", "existing-repo", "--type", "ctex-md", "--no-commit"],
    ));
    assert!(target.join(".omnidoc.toml").is_file());
    assert!(target.join("md/notes.md").is_file());
    let repository = git2::Repository::open(&target).expect("open initialized repository");
    assert_eq!(
        repository.head().expect("repository head").target(),
        Some(initial_commit)
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
fn update_commit_records_tracked_source_moves_as_deletions_and_additions() {
    let fixture = Fixture::new("update-tracked-move");
    fs::write(fixture.project.join("notes.md"), "# Tracked notes\n").expect("tracked notes");
    let repository = git2::Repository::init(&fixture.project).expect("initialize repository");
    let mut index = repository.index().expect("repository index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("stage baseline");
    index.write().expect("persist baseline index");
    let tree_id = index.write_tree().expect("write baseline tree");
    let tree = repository.find_tree(tree_id).expect("baseline tree");
    let signature =
        git2::Signature::now("OmniDoc Test", "omnidoc@example.invalid").expect("signature");
    repository
        .commit(Some("HEAD"), &signature, &signature, "Baseline", &tree, &[])
        .expect("baseline commit");
    drop(tree);
    drop(repository);

    assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));

    let repository = git2::Repository::open(&fixture.project).expect("updated repository");
    let tree = repository
        .head()
        .expect("updated head")
        .peel_to_commit()
        .expect("updated commit")
        .tree()
        .expect("updated tree");
    assert!(tree.get_path(Path::new("notes.md")).is_err());
    assert!(tree.get_path(Path::new("md/notes.md")).is_ok());
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
fn update_skips_noop_commits_and_reports_only_real_changes() {
    let fixture = Fixture::new("update-noop");
    assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let repository = git2::Repository::open(&fixture.project).expect("updated repository");
    let first_head = repository
        .head()
        .expect("first head")
        .target()
        .expect("head id");
    drop(repository);

    let output =
        assert_success(fixture.command(&["update", "--dry-run", "--json", &fixture.project_arg()]));
    let preview: serde_json::Value = serde_json::from_str(&output).expect("no-op preview JSON");
    assert_eq!(preview["ready"], true);
    assert_eq!(preview["repository"]["clean"], true);
    assert_eq!(preview["actions"], serde_json::json!([]));

    let output = assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("no-op update JSON");
    assert_eq!(report["actions"], serde_json::json!([]));
    assert_eq!(report["will_commit"], false);
    let repository = git2::Repository::open(&fixture.project).expect("repository after no-op");
    assert_eq!(
        repository.head().expect("head after no-op").target(),
        Some(first_head)
    );
    drop(repository);

    fs::write(fixture.project.join("main.md"), "# Existing user change\n")
        .expect("dirty unrelated source");
    let output = assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("dirty no-op update JSON");
    assert_eq!(report["ready"], true);
    assert_eq!(report["will_commit"], false);
    assert_eq!(report["actions"], serde_json::json!([]));
    let repository = git2::Repository::open(&fixture.project).expect("dirty no-op repository");
    assert_eq!(
        repository.head().expect("dirty no-op head").target(),
        Some(first_head)
    );
    assert_eq!(
        fs::read_to_string(fixture.project.join("main.md")).expect("preserved user change"),
        "# Existing user change\n"
    );
}

#[test]
fn update_diff_is_a_read_only_managed_file_preview() {
    let fixture = Fixture::new("update-diff");
    assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let gitignore = fixture.project.join(".gitignore");
    let mut customized = fs::read_to_string(&gitignore).expect("managed gitignore");
    customized.push_str("\n# local rule\ncustom-output/\n");
    fs::write(&gitignore, &customized).expect("customized gitignore");

    let output = assert_success(fixture.command(&[
        "update",
        "--diff",
        "--no-commit",
        "--json",
        &fixture.project_arg(),
    ]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("update diff JSON");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["diff"], true);
    assert_eq!(report["applied"], false);
    assert_eq!(report["ready"], true);
    let action = report["actions"]
        .as_array()
        .expect("diff actions")
        .iter()
        .find(|action| {
            action["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/.gitignore"))
        })
        .expect("gitignore action");
    assert_eq!(action["operation"], "refresh_file");
    assert_eq!(action["change"], "update");
    assert!(action["diff"]
        .as_str()
        .is_some_and(|diff| diff.contains("--- a/.gitignore")
            && diff.contains("+++ b/.gitignore")
            && diff.contains("-# local rule")));
    assert_eq!(
        fs::read_to_string(&gitignore).expect("preserved customized gitignore"),
        customized
    );
}

#[test]
fn update_refuses_to_mix_existing_changes_into_automatic_commit() {
    let fixture = Fixture::new("update-dirty-repo");
    assert_success(fixture.command(&["update", "--json", &fixture.project_arg()]));
    let repository = git2::Repository::open(&fixture.project).expect("updated repository");
    let original_head = repository
        .head()
        .expect("original head")
        .target()
        .expect("original head id");
    drop(repository);
    fs::write(fixture.project.join("main.md"), "# User work\n").expect("dirty source");
    let gitignore = fixture.project.join(".gitignore");
    let mut customized_gitignore = fs::read_to_string(&gitignore).expect("managed gitignore");
    customized_gitignore.push_str("\n# local customization\n");
    fs::write(&gitignore, &customized_gitignore).expect("dirty managed file");

    let preview =
        assert_success(fixture.command(&["update", "--dry-run", "--json", &fixture.project_arg()]));
    let preview: serde_json::Value = serde_json::from_str(&preview).expect("dirty preview JSON");
    assert_eq!(preview["ready"], false);
    assert_eq!(preview["repository"]["exists"], true);
    assert_eq!(preview["repository"]["has_commits"], true);
    assert_eq!(preview["repository"]["clean"], false);
    assert_eq!(preview["will_commit"], true);
    assert!(preview["repository"]["changes"]
        .as_array()
        .is_some_and(|changes| changes
            .iter()
            .any(|change| { change["path"] == "main.md" && change["worktree"] == "modified" })));

    let output = fixture.command(&["update", "--json", &fixture.project_arg()]);
    let stdout = assert_failure(output);
    let error: serde_json::Value = serde_json::from_str(&stdout).expect("dirty update error JSON");
    assert!(error["error"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("--no-commit")));
    assert_eq!(
        fs::read_to_string(fixture.project.join("main.md")).expect("preserved source"),
        "# User work\n"
    );
    assert_eq!(
        fs::read_to_string(&gitignore).expect("preserved managed file"),
        customized_gitignore
    );
    let repository = git2::Repository::open(&fixture.project).expect("repository after refusal");
    assert_eq!(
        repository.head().expect("head after refusal").target(),
        Some(original_head)
    );
    drop(repository);

    let output = assert_success(fixture.command(&[
        "update",
        "--no-commit",
        "--json",
        &fixture.project_arg(),
    ]));
    let report: serde_json::Value = serde_json::from_str(&output).expect("no-commit JSON");
    assert_eq!(report["ready"], true);
    assert_eq!(report["commit"], false);
    assert_eq!(report["will_commit"], false);
    assert!(!fs::read_to_string(&gitignore)
        .expect("refreshed managed file")
        .contains("local customization"));
    assert_eq!(
        fs::read_to_string(fixture.project.join("main.md")).expect("preserved user work"),
        "# User work\n"
    );
    let repository = git2::Repository::open(&fixture.project).expect("no-commit repository");
    assert_eq!(
        repository.head().expect("head after no-commit").target(),
        Some(original_head)
    );
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
fn config_set_previews_and_applies_typed_comment_preserving_edits() {
    let fixture = Fixture::new("config-set");
    let project = fixture.project_arg();
    let config_path = fixture.project.join(".omnidoc.toml");
    fs::write(
        &config_path,
        concat!(
            "# Keep project guidance\n",
            "[project]\n",
            "entry = \"main.md\"\n",
            "from = \"markdown\"\n",
            "to = \"html\"\n",
            "target = 'smoke' # artifact basename\n",
            "\n",
            "[build]\n",
            "outdir = \"build\"\n",
            "outputs = [\"html\"]\n",
        ),
    )
    .expect("commented config");
    let before = fs::read_to_string(&config_path).expect("before config");

    let preview = assert_success(fixture.command(&[
        "config",
        "set",
        "project.target",
        "guide",
        &project,
        "--diff",
        "--json",
    ]));
    let preview: serde_json::Value = serde_json::from_str(&preview).expect("set preview JSON");
    assert_eq!(preview["schema_version"], 1);
    assert_eq!(preview["operation"], "set");
    assert_eq!(preview["scope"], "project");
    assert_eq!(preview["previous"], "smoke");
    assert_eq!(preview["value"], "guide");
    assert_eq!(preview["changed"], true);
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["applied"], false);
    assert!(preview["diff"]
        .as_str()
        .is_some_and(|diff| diff.contains("target = \"guide\"")));
    assert_eq!(
        fs::read_to_string(&config_path).expect("preview config"),
        before
    );
    assert!(!fixture.project.join(".omnidoc-cache").exists());

    let applied = assert_success(fixture.command(&[
        "config",
        "set",
        "project.target",
        "guide",
        &project,
        "--json",
    ]));
    let applied: serde_json::Value = serde_json::from_str(&applied).expect("set JSON");
    assert_eq!(applied["changed"], true);
    assert_eq!(applied["applied"], true);
    let edited = fs::read_to_string(&config_path).expect("edited config");
    assert!(edited.contains("# Keep project guidance"));
    assert!(edited.contains("target = \"guide\" # artifact basename"));

    assert_success(fixture.command(&[
        "config",
        "set",
        "build.outputs",
        "[\"pdf\", \"html\"]",
        &project,
    ]));
    assert_success(fixture.command(&["config", "set", "build.verbose", "true", &project]));
    let shown = assert_success(
        fixture.command(&["config", "show", &project, "--scope", "project", "--json"]),
    );
    let shown: serde_json::Value = serde_json::from_str(&shown).expect("project config JSON");
    assert_eq!(shown["config"]["project"]["target"], "guide");
    assert_eq!(
        shown["config"]["build"]["outputs"],
        serde_json::json!(["pdf", "html"])
    );
    assert_eq!(shown["config"]["build"]["verbose"], true);

    let no_op = assert_success(fixture.command(&[
        "config",
        "set",
        "project.target",
        "guide",
        &project,
        "--json",
    ]));
    let no_op: serde_json::Value = serde_json::from_str(&no_op).expect("no-op JSON");
    assert_eq!(no_op["changed"], false);
    assert_eq!(no_op["applied"], false);

    let string_like = assert_success(fixture.command(&[
        "config",
        "set",
        "project.target",
        "2026-07-27",
        &project,
        "--json",
    ]));
    let string_like: serde_json::Value =
        serde_json::from_str(&string_like).expect("schema-aware string JSON");
    assert_eq!(string_like["value"], "2026-07-27");
}

#[test]
fn config_unset_is_safe_and_global_set_bootstraps_complete_defaults() {
    let fixture = Fixture::new("config-unset-global");
    let project = fixture.project_arg();
    let project_before =
        fs::read_to_string(fixture.project.join(".omnidoc.toml")).expect("project config");
    let global_path = fixture.env_root.join("config/omnidoc.toml");
    fs::remove_file(&global_path).ok();

    let global = assert_success(fixture.command(&[
        "config",
        "set",
        "author.name",
        "Docs Team",
        "--scope",
        "global",
        "--json",
    ]));
    let global: serde_json::Value = serde_json::from_str(&global).expect("global set JSON");
    assert_eq!(global["created"], true);
    assert_eq!(global["applied"], true);
    assert_eq!(global["value"], "Docs Team");
    let global_config = fs::read_to_string(&global_path).expect("global config");
    assert!(global_config.contains("name = \"Docs Team\""));
    assert!(global_config.contains("[lib]"));
    assert!(global_config.contains("[env]"));
    assert_eq!(
        fs::read_to_string(fixture.project.join(".omnidoc.toml"))
            .expect("unchanged project config"),
        project_before
    );

    let preview = assert_success(fixture.command(&[
        "config",
        "unset",
        "env.texinputs",
        "--scope",
        "global",
        "--dry-run",
        "--json",
    ]));
    let preview: serde_json::Value = serde_json::from_str(&preview).expect("unset preview JSON");
    assert_eq!(preview["previous"], "./tex//:");
    assert_eq!(preview["value"], serde_json::Value::Null);
    assert_eq!(preview["changed"], true);
    assert_eq!(preview["applied"], false);
    assert_eq!(
        fs::read_to_string(&global_path).expect("preview global config"),
        global_config
    );

    assert_success(fixture.command(&["config", "unset", "env.texinputs", "--scope", "global"]));
    let updated = fs::read_to_string(&global_path).expect("updated global config");
    assert!(!updated.contains("texinputs"));

    assert_success(fixture.command(&[
        "config",
        "set",
        "paths.build_dir",
        "global-build",
        "--scope",
        "global",
    ]));
    assert_success(fixture.command(&[
        "config",
        "set",
        "paths.build_dir",
        "project-build",
        &project,
    ]));
    let merged = assert_success(fixture.command(&["config", "show", &project, "--json"]));
    let merged: serde_json::Value = serde_json::from_str(&merged).expect("merged config JSON");
    assert_eq!(merged["config"]["paths"]["build_dir"], "project-build");

    let no_op =
        assert_success(fixture.command(&["config", "unset", "theme.name", &project, "--json"]));
    let no_op: serde_json::Value = serde_json::from_str(&no_op).expect("unset no-op JSON");
    assert_eq!(no_op["changed"], false);
    assert_eq!(no_op["applied"], false);
}

#[test]
fn config_writes_reject_invalid_types_keys_and_ignored_scopes_without_changes() {
    let fixture = Fixture::new("config-write-errors");
    let project = fixture.project_arg();
    let config_path = fixture.project.join(".omnidoc.toml");
    let before = fs::read_to_string(&config_path).expect("before config");

    for args in [
        vec![
            "config",
            "set",
            "build.outputs",
            "not-an-array",
            &project,
            "--json",
        ],
        vec![
            "config",
            "set",
            "build.outputs",
            "[\"pdf\", \"exe\"]",
            &project,
            "--json",
        ],
        vec!["config", "set", "build.outptuz", "html", &project, "--json"],
        vec!["config", "set", "theme.version", "1", &project, "--json"],
        vec![
            "config",
            "set",
            "build.outputs",
            "[\"html\"]",
            "--scope",
            "global",
            "--json",
        ],
    ] {
        let output = fixture.command(&args);
        assert!(!output.status.success());
        let error: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("configuration error JSON");
        assert_eq!(error["error"]["category"], "configuration");
        assert_eq!(
            fs::read_to_string(&config_path).expect("unchanged config"),
            before
        );
    }

    assert!(!fixture.env_root.join("config/omnidoc.toml").exists());
    assert!(!fixture.project.join(".omnidoc-cache").exists());
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

    let listed = assert_success(fixture.command(&["plugin", "list", &project, "--json"]));
    assert!(listed.contains("\"lint_rule\""));

    let plugins = assert_success(fixture.command(&["plugin", "validate", &project, "--json"]));
    assert!(plugins.contains("\"lint_rule\""));
}
