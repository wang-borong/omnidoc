use git2::{Repository, Signature};
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

    let repository = Repository::init(&library).expect("library repository");
    let mut index = repository.index().expect("library index");
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .expect("add library files");
    let tree_id = index.write_tree().expect("library tree");
    let tree = repository.find_tree(tree_id).expect("find library tree");
    let signature =
        Signature::now("OmniDoc Test", "omnidoc@example.invalid").expect("library signature");
    let revision = repository
        .commit(
            Some("HEAD"),
            &signature,
            &signature,
            "library fixture",
            &tree,
            &[],
        )
        .expect("library commit")
        .to_string();
    fs::write(
        fixture.env_root.join("config/omnidoc.toml"),
        format!(
            "[lib]\npath = {:?}\nrevision = {:?}\n",
            library.to_string_lossy(),
            revision
        ),
    )
    .expect("pinned global config");

    let verified = assert_success(fixture.command(&["libs", "--verify", "--json"]));
    assert!(verified.contains("\"version\": \"1.0.0\""));
    assert!(verified.contains("\"integrity_verified\": true"));
    assert!(verified.contains("\"revision_matches\": true"));
    assert!(verified.contains(&revision));

    let mismatch =
        assert_failure(fixture.command(&["libs", "--verify", "--json", "--revision", "deadbeef"]));
    assert!(mismatch.contains("\"revision_matches\": null"));
    assert!(mismatch.contains("cannot resolve requested revision deadbeef"));

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
fn json_commands_remain_machine_readable_when_creating_default_config() {
    let fixture = Fixture::new("json-default-config");
    fs::remove_file(fixture.env_root.join("config/omnidoc.toml")).ok();

    let output = fixture.command(&["theme", "list", "--json"]);
    let stdout = assert_success(output);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("pure JSON stdout");
    assert_eq!(parsed, serde_json::json!([]));
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
    assert!(text.contains("正文中文 ABC"));
    assert!(text.contains("``code:中文ABC``"));

    assert_success(fixture.command(&["fmt", &path]));
    let twice = fs::read(&markdown).expect("formatted twice");
    assert_eq!(once, twice);
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
    assert_eq!(manifest["library_contract"]["library"]["version"], "1.0.0");
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
