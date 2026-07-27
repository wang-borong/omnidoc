use crate::doctype::DocumentFormat;
use clap::{Parser, Subcommand, ValueEnum, ValueHint};

/// The OmniDoc management CLI
#[derive(Debug, Parser)]
#[command(name = "omnidoc")]
#[command(
    version,
    about = "Create, build, validate, and publish document projects"
)]
#[command(arg_required_else_help = true, propagate_version = true)]
#[command(
    after_help = "Quick start:\n  omnidoc new my-book --type ctex-md\n  omnidoc build my-book\n  omnidoc status my-book\n\nWorkflow groups:\n  omnidoc check --help       Project validation and CI\n  omnidoc convert --help     Standalone format conversion\n  omnidoc template --help    Template discovery and validation\n  omnidoc plugin --help      Plugin examples and project hooks\n  omnidoc lib --help         Managed library lifecycle\n\nLegacy flat command forms remain supported for scripts."
)]
pub struct OmniCli {
    /// document management subcommands
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// create a new OmniDoc project
    #[command(visible_alias = "create")]
    #[command(
        after_help = "Examples:\n  omnidoc new my-book\n  omnidoc new my-book --type ctex-md\n  omnidoc new my-book --defaults\n  omnidoc new report --format latex\n  omnidoc new my-book --type ctex-md --dry-run\n  omnidoc new my-book --type ctex-md --json"
    )]
    New {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: Option<String>,

        /// use a template key directly and skip the selector
        #[arg(
            short = 'T',
            long = "type",
            visible_alias = "template",
            alias = "doctype",
            value_name = "KEY"
        )]
        doctype: Option<String>,

        /// limit template selection to one source format
        #[arg(long, value_enum)]
        format: Option<DocumentFormat>,

        /// accept recommended defaults (ctex-md) without prompting
        #[arg(short = 'y', long, visible_alias = "yes", conflicts_with_all = ["doctype", "format"])]
        defaults: bool,

        /// preview the resolved project plan without creating files
        #[arg(long)]
        dry_run: bool,

        /// initialize Git but leave generated files uncommitted
        #[arg(long)]
        no_commit: bool,

        /// emit a stable JSON creation report
        #[arg(long)]
        json: bool,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: String,
    },

    /// initialize an existing directory as an OmniDoc project
    #[command(
        after_help = "Examples:\n  omnidoc init\n  omnidoc init . --type ctex-md\n  omnidoc init existing-repo --defaults\n  omnidoc init existing-repo --type ctex-md --dry-run\n  omnidoc init existing-repo --type ctex-md --diff\n  omnidoc init existing-repo --type ctex-md --no-commit"
    )]
    Init {
        /// set the author name
        #[arg(short, long)]
        author: Option<String>,

        /// set the document title
        #[arg(short = 't', long)]
        title: Option<String>,

        /// use a template key directly and skip the selector
        #[arg(
            short = 'T',
            long = "type",
            visible_alias = "template",
            alias = "doctype",
            value_name = "KEY"
        )]
        doctype: Option<String>,

        /// limit template selection to one source format
        #[arg(long, value_enum)]
        format: Option<DocumentFormat>,

        /// accept recommended defaults (ctex-md) without prompting
        #[arg(short = 'y', long, visible_alias = "yes", conflicts_with_all = ["doctype", "format"])]
        defaults: bool,

        /// initialize files without creating a Git commit
        #[arg(long)]
        no_commit: bool,

        /// preview the initialization plan without modifying the directory
        #[arg(long)]
        dry_run: bool,

        /// show unified managed-file diffs and imply --dry-run
        #[arg(long)]
        diff: bool,

        /// emit a stable JSON initialization report
        #[arg(long)]
        json: bool,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// build the document project
    Build {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, pptx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend")]
        latex_backend: Option<String>,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// write build/omnidoc-report.json
        #[arg(long)]
        report: bool,

        /// update omnidoc.lock after a successful build
        #[arg(long = "write-lock")]
        write_lock: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose message
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// watch a document project and rebuild on source changes
    Watch {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, pptx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend")]
        latex_backend: Option<String>,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// debounce interval in milliseconds
        #[arg(long = "debounce-ms", default_value_t = 250)]
        debounce_ms: u64,

        /// run the initial build and exit after one scan cycle
        #[arg(long)]
        once: bool,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// write build/omnidoc-report.json
        #[arg(long)]
        report: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose build messages
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// build and publish generated artifacts into a dist directory
    Publish {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// override output format (pdf, html, epub, docx, pptx, latex)
        #[arg(long)]
        to: Option<String>,

        /// build/publish all configured or default outputs
        #[arg(long)]
        all: bool,

        /// build/publish multiple output formats (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,

        /// override PDF engine (xelatex, lualatex, pdflatex, tectonic, or executable path)
        #[arg(long = "pdf-engine")]
        pdf_engine: Option<String>,

        /// LaTeX project backend (latexmk or engine)
        #[arg(long = "latex-backend")]
        latex_backend: Option<String>,

        /// maximum direct LaTeX engine passes for --latex-backend engine
        #[arg(long = "max-latex-passes")]
        max_latex_passes: Option<usize>,

        /// publish directory
        #[arg(long = "dist-dir", default_value = "dist")]
        dist_dir: String,

        /// publish tag or release directory name
        #[arg(long)]
        tag: Option<String>,

        /// copy existing build artifacts without rebuilding first
        #[arg(long = "no-build")]
        no_build: bool,

        /// verify an existing published release instead of building or copying
        #[arg(long, requires = "tag")]
        verify: bool,

        /// emit publish verification results as JSON
        #[arg(long, requires = "verify")]
        json: bool,

        /// force rebuild even when input cache is unchanged
        #[arg(short = 'F', long)]
        force: bool,

        /// fail on lint warnings before build
        #[arg(long)]
        strict: bool,

        /// show verbose build messages
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// validate, inspect, lock, and test a project
    Check {
        #[command(subcommand)]
        subcommand: CheckSubcommand,
    },

    /// diagnose local tools, configuration, and template library
    Doctor {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON diagnostics
        #[arg(long)]
        json: bool,

        /// return a non-zero exit status when any check fails
        #[arg(long)]
        strict: bool,

        /// diagnose one or more output formats instead of every configured output
        #[arg(long = "output", value_name = "FORMAT")]
        outputs: Vec<String>,
    },

    /// validate OmniDoc configuration files
    #[command(hide = true)]
    ConfigValidate {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// lint document sources for missing resources and weak references
    #[command(hide = true)]
    Lint {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// treat warnings as errors
        #[arg(long)]
        strict: bool,
    },

    /// print the tracked project dependency graph
    #[command(hide = true)]
    Deps {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON dependency graph
        #[arg(long)]
        json: bool,
    },

    /// run strict CI checks and configured builds
    #[command(hide = true)]
    Ci {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// output format to build (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,
    },

    /// create or update omnidoc.lock
    #[command(hide = true)]
    Lock {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// check whether omnidoc.lock matches current project inputs
        #[arg(long, conflicts_with = "update")]
        check: bool,

        /// rewrite the lock file
        #[arg(long)]
        update: bool,
    },

    /// discover, install, and validate project plugins
    #[command(
        args_conflicts_with_subcommands = true,
        after_help = "Examples:\n  omnidoc plugin examples\n  omnidoc plugin add quality-gate docs\n  omnidoc plugin add asset-index docs --dry-run\n  omnidoc plugin list docs\n  omnidoc plugin validate docs --json\n\nBundled examples are inert until `plugin add` copies one into a project's `plugins/` directory. The legacy `omnidoc plugin [PATH] --validate` form remains supported."
    )]
    Plugin {
        #[command(subcommand)]
        subcommand: Option<PluginSubcommand>,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath, hide = true)]
        path: Option<String>,

        /// emit JSON plugin metadata
        #[arg(long, hide = true)]
        json: bool,

        /// validate discovered plugin/template manifests
        #[arg(long, hide = true)]
        validate: bool,
    },

    /// show resolved project paths, configuration, and build artifacts
    #[command(visible_alias = "info")]
    Status {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit stable JSON status
        #[arg(long)]
        json: bool,
    },

    /// open a built document in the system viewer
    #[command(
        after_help = "Examples:\n  omnidoc open\n  omnidoc open --to html\n  omnidoc open --print-path\n  omnidoc open docs --to epub --print-path"
    )]
    Open {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// select the output format (pdf, html, epub, docx, pptx, latex)
        #[arg(long, visible_alias = "format", value_name = "FORMAT")]
        to: Option<String>,

        /// print the resolved artifact path without launching a viewer
        #[arg(long)]
        print_path: bool,
    },

    /// preview or remove generated build artifacts
    #[command(
        after_help = "Examples:\n  omnidoc clean --dry-run\n  omnidoc clean --dry-run --json\n  omnidoc clean\n  omnidoc clean --distclean"
    )]
    Clean {
        /// also remove known root-level temporary files and the auto directory
        #[arg(short = 'D', long)]
        distclean: bool,

        /// report exactly what would be removed without modifying the project
        #[arg(long)]
        dry_run: bool,

        /// emit a stable JSON clean report
        #[arg(long)]
        json: bool,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// preview or refresh project scaffolding for the current OmniDoc version
    #[command(
        after_help = "Examples:\n  omnidoc update --dry-run\n  omnidoc update --diff\n  omnidoc update --dry-run --json\n  omnidoc update --no-commit\n  omnidoc update"
    )]
    Update {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// report the update plan without modifying the project
        #[arg(long)]
        dry_run: bool,

        /// show unified managed-file diffs and imply --dry-run
        #[arg(long)]
        diff: bool,

        /// apply the update without creating a Git commit
        #[arg(long)]
        no_commit: bool,

        /// emit a stable JSON update report
        #[arg(long)]
        json: bool,
    },

    /// inspect, create, or safely update OmniDoc configuration
    #[command(
        after_help = "Examples:\n  omnidoc config show --json\n  omnidoc config get target\n  omnidoc config set project.target guide\n  omnidoc config set build.outputs '[\"pdf\", \"html\"]' --dry-run\n  omnidoc config unset theme.name\n  omnidoc config init --author 'Docs Team'\n\nThe legacy `omnidoc config --authors NAME` form remains supported."
    )]
    Config {
        #[command(subcommand)]
        subcommand: Option<ConfigSubcommand>,

        /// configure the author name
        #[arg(short, long = "author", visible_alias = "authors", hide = true)]
        authors: Option<String>,
        /// configure the OmniDoc library path
        #[arg(short, long, hide = true)]
        lib: Option<String>,
        /// configure the output directory for building
        #[arg(short, long, hide = true)]
        outdir: Option<String>,
        /// configure the TEXMFHOME environment variable (the directory where the system finds the texmf home)
        #[arg(short = 'T', long, hide = true)]
        texmfhome: Option<String>,
        /// configure the BIBINPUTS environment variable (the directory where the system finds the bibliographies)
        #[arg(short, long, hide = true)]
        bibinputs: Option<String>,
        /// configure the TEXINPUTS environment variable (the directory where the system finds the tex sources)
        #[arg(short, long, hide = true)]
        texinputs: Option<String>,

        /// force generation
        #[arg(short = 'F', long, hide = true)]
        force: bool,
    },

    /// install, inspect, update, and verify the OmniDoc library
    #[command(
        visible_alias = "libs",
        args_conflicts_with_subcommands = true,
        after_help = "Examples:\n  omnidoc lib install\n  omnidoc lib status --json\n  omnidoc lib verify\n  omnidoc lib update\n\nThe legacy `omnidoc lib --install|--update|--status|--verify` forms remain supported."
    )]
    Lib {
        #[command(subcommand)]
        subcommand: Option<LibSubcommand>,

        /// install the release-bound OmniDoc library to XDG_DATA_DIR
        #[arg(short, long, conflicts_with_all = ["update", "status", "verify"], hide = true)]
        install: bool,

        /// update the OmniDoc library from the release bound to this version
        #[arg(short, long, conflicts_with_all = ["install", "status", "verify"], hide = true)]
        update: bool,

        /// show installed library version, release, compatibility, and integrity
        #[arg(long, conflicts_with_all = ["install", "update", "verify"], hide = true)]
        status: bool,

        /// verify the installed manifest, required resources, and checksums
        #[arg(long, conflicts_with_all = ["install", "update", "status"], hide = true)]
        verify: bool,

        /// emit status or verification details as JSON
        #[arg(long, hide = true)]
        json: bool,
    },

    /// discover, validate, and select versioned theme bundles
    #[command(
        after_help = "Examples:\n  omnidoc theme list\n  omnidoc theme inspect corporate-docs\n  omnidoc theme apply corporate-docs ./docs\n  omnidoc theme apply modern-slides ./talk --dry-run\n  omnidoc theme validate --json"
    )]
    Theme {
        #[command(subcommand)]
        subcommand: ThemeSubcommand,
    },

    /// list all supported document types
    #[command(hide = true)]
    List,

    /// discover and validate built-in and external project templates
    Template {
        #[command(subcommand)]
        subcommand: Option<TemplateSubcommand>,

        /// validate external template manifests & files
        #[arg(long, hide = true)]
        validate: bool,
    },

    /// generate shell completion
    #[command(visible_aliases = ["completion", "completions"])]
    Complete {
        /// shell to generate completion for
        #[arg(value_enum, value_name = "SHELL", conflicts_with = "generator")]
        shell: Option<clap_complete::Shell>,

        /// legacy form of the shell selection option
        #[arg(short, long = "generate", value_enum, hide = true)]
        generator: Option<clap_complete::Shell>,
    },

    /// convert standalone Markdown files without creating a project
    Convert {
        #[command(subcommand)]
        subcommand: ConvertSubcommand,
    },

    /// convert markdown files to PDF
    #[command(hide = true)]
    Md2pdf {
        /// language (cn or en)
        #[arg(short, long)]
        lang: Option<String>,
        /// output file path
        #[arg(short, long)]
        output: Option<String>,

        /// input markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },

    /// convert markdown files to HTML
    #[command(hide = true)]
    Md2html {
        /// output file path (for single input) or directory (for multiple inputs)
        #[arg(short, long)]
        output: Option<String>,

        /// CSS file path
        #[arg(short, long)]
        css: Option<String>,

        /// input markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },

    /// format documents (recursively format directories or format files)
    #[command(visible_alias = "format")]
    Fmt {
        /// create backup files
        #[arg(short, long, conflicts_with_all = ["check", "diff"])]
        backup: bool,

        /// report files that would change without writing them
        #[arg(long, conflicts_with = "diff")]
        check: bool,

        /// print unified diffs without writing files
        #[arg(long, conflicts_with = "check")]
        diff: bool,

        /// enable semantic formatting
        #[arg(short, long)]
        semantic: bool,

        /// enable symbol formatting (Chinese punctuation)
        #[arg(short = 'y', long)]
        symbol: bool,

        /// paths to format (files or directories)
        #[arg(value_hint = ValueHint::AnyPath)]
        paths: Vec<String>,
    },

    /// generate figures from source files
    Figure {
        #[command(subcommand)]
        subcommand: Option<FigureSubcommand>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,

        /// source figure files (auto-detect type if no subcommand specified)
        #[arg(value_hint = ValueHint::FilePath)]
        sources: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigScope {
    /// effective command configuration after global/project merging
    Merged,
    /// user-level configuration
    Global,
    /// nearest .omnidoc.toml project configuration
    Project,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ConfigWriteScope {
    /// user-level configuration
    Global,
    /// nearest .omnidoc.toml project configuration
    Project,
}

#[derive(Debug, Subcommand)]
pub enum ConfigSubcommand {
    /// create the user-level configuration file explicitly
    Init {
        /// configure the author name
        #[arg(short, long, visible_alias = "authors")]
        author: String,

        /// configure the OmniDoc library path
        #[arg(short, long)]
        lib: Option<String>,

        /// configure the output directory for building
        #[arg(short, long)]
        outdir: Option<String>,

        /// configure the TEXMFHOME environment variable
        #[arg(short = 'T', long)]
        texmfhome: Option<String>,

        /// configure the BIBINPUTS environment variable
        #[arg(short, long)]
        bibinputs: Option<String>,

        /// configure the TEXINPUTS environment variable
        #[arg(short, long)]
        texinputs: Option<String>,

        /// overwrite an existing configuration file
        #[arg(short = 'F', long)]
        force: bool,
    },

    /// show a complete configuration scope
    Show {
        /// project path used to resolve project/merged configuration
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// configuration scope to inspect
        #[arg(long, value_enum, default_value = "merged")]
        scope: ConfigScope,

        /// emit a stable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// read one dot-separated configuration key
    Get {
        /// key such as target, outdir, project.target, or tools.pandoc
        key: String,

        /// project path used to resolve project/merged configuration
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// configuration scope to inspect
        #[arg(long, value_enum, default_value = "merged")]
        scope: ConfigScope,

        /// emit a stable JSON envelope
        #[arg(long)]
        json: bool,
    },

    /// set one dot-separated configuration key
    #[command(
        after_help = "Examples:\n  omnidoc config set project.target guide\n  omnidoc config set build.outputs '[\"pdf\", \"html\"]'\n  omnidoc config set author.name 'Docs Team' --scope global\n  omnidoc config set project.to html --diff\n\nValues follow the configuration schema: booleans, integers, and arrays use TOML syntax, while string keys accept natural unquoted text."
    )]
    Set {
        /// key such as project.target, build.outputs, or tools.pandoc
        key: String,

        /// value to store; the schema determines string, boolean, integer, or array parsing
        value: String,

        /// project path used to resolve project configuration
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// configuration file to update
        #[arg(long, value_enum, default_value = "project")]
        scope: ConfigWriteScope,

        /// report the change without writing the configuration file
        #[arg(long)]
        dry_run: bool,

        /// show a unified diff and imply --dry-run
        #[arg(long)]
        diff: bool,

        /// emit a stable JSON change report
        #[arg(long)]
        json: bool,
    },

    /// remove one dot-separated configuration key or section
    #[command(
        after_help = "Examples:\n  omnidoc config unset theme.name\n  omnidoc config unset pandoc.format_options.html\n  omnidoc config unset tools.pandoc --scope global --dry-run\n  omnidoc config unset theme --diff"
    )]
    Unset {
        /// key or section such as theme.name, tools.pandoc, or pandoc
        key: String,

        /// project path used to resolve project configuration
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// configuration file to update
        #[arg(long, value_enum, default_value = "project")]
        scope: ConfigWriteScope,

        /// report the change without writing the configuration file
        #[arg(long)]
        dry_run: bool,

        /// show a unified diff and imply --dry-run
        #[arg(long)]
        diff: bool,

        /// emit a stable JSON change report
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum LibSubcommand {
    /// install the release-bound OmniDoc library
    Install {
        /// emit installed library details as JSON
        #[arg(long)]
        json: bool,
    },

    /// update the library from the release bound to this OmniDoc version
    Update {
        /// emit updated library details as JSON
        #[arg(long)]
        json: bool,
    },

    /// show installed version, compatibility, and integrity
    Status {
        /// emit stable JSON library details
        #[arg(long)]
        json: bool,
    },

    /// verify the installed manifest, required resources, and checksums
    Verify {
        /// emit stable JSON verification details
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum PluginSubcommand {
    /// list bundled plugin examples that can be installed explicitly
    Examples {
        /// optional project path used to resolve the configured library
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit stable JSON example metadata
        #[arg(long)]
        json: bool,
    },

    /// install one bundled example into a project's plugins directory
    Add {
        /// bundled example key, such as quality-gate or asset-index
        preset: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// report the installation without writing files
        #[arg(long)]
        dry_run: bool,

        /// emit a stable JSON installation report
        #[arg(long)]
        json: bool,
    },

    /// list discovered plugins and external templates
    List {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON plugin metadata
        #[arg(long)]
        json: bool,
    },

    /// validate discovered plugin and template manifests
    Validate {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON validation metadata
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum CheckSubcommand {
    /// diagnose local tools, configuration, and template library
    Doctor {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON diagnostics
        #[arg(long)]
        json: bool,

        /// return a non-zero exit status when any check fails
        #[arg(long)]
        strict: bool,

        /// diagnose one or more output formats instead of every configured output
        #[arg(long = "output", value_name = "FORMAT")]
        outputs: Vec<String>,
    },

    /// validate OmniDoc configuration files
    #[command(visible_alias = "validate")]
    Config {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,
    },

    /// lint document sources for missing resources and weak references
    Lint {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// treat warnings as errors
        #[arg(long)]
        strict: bool,
    },

    /// print the tracked project dependency graph
    Deps {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// emit JSON dependency graph
        #[arg(long)]
        json: bool,
    },

    /// create, update, or verify omnidoc.lock
    Lock {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// check whether omnidoc.lock matches current project inputs
        #[arg(long, conflicts_with = "update")]
        check: bool,

        /// rewrite the lock file
        #[arg(long)]
        update: bool,
    },

    /// run strict CI checks and configured builds
    Ci {
        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// output format to build (repeatable)
        #[arg(long = "output")]
        outputs: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum TemplateSubcommand {
    /// list template keys accepted by new and init
    List {
        /// filter templates by source format
        #[arg(long, value_enum)]
        format: Option<DocumentFormat>,

        /// emit stable JSON metadata
        #[arg(long)]
        json: bool,
    },

    /// validate external template manifests and rendering
    Validate {
        /// validate only one template key
        key: Option<String>,

        /// emit validation results as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConvertSubcommand {
    /// convert Markdown files to PDF
    Pdf {
        /// language (cn or en)
        #[arg(short, long)]
        lang: Option<String>,

        /// output file path
        #[arg(short, long)]
        output: Option<String>,

        /// input Markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },

    /// convert Markdown files to HTML
    Html {
        /// output file path (for single input) or directory (for multiple inputs)
        #[arg(short, long)]
        output: Option<String>,

        /// CSS file path
        #[arg(short, long)]
        css: Option<String>,

        /// input Markdown files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        inputs: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ThemeSubcommand {
    /// list installed theme bundles
    List {
        /// emit JSON theme metadata
        #[arg(long)]
        json: bool,
    },

    /// inspect one installed theme bundle
    Inspect {
        /// theme name
        name: String,

        /// emit JSON theme metadata
        #[arg(long)]
        json: bool,
    },

    /// validate one theme, or every installed theme when NAME is omitted
    Validate {
        /// optional theme name
        name: Option<String>,

        /// emit JSON validation results
        #[arg(long)]
        json: bool,

        /// verify required font families with fontconfig
        #[arg(long)]
        check_fonts: bool,

        /// verify required system LaTeX packages with kpsewhich
        #[arg(long)]
        check_latex: bool,
    },

    /// select an installed theme for a project
    #[command(visible_alias = "use")]
    Apply {
        /// installed theme name
        name: String,

        /// set the path to a documentation project
        #[arg(value_hint = ValueHint::DirPath)]
        path: Option<String>,

        /// report the configuration change without writing it
        #[arg(long)]
        dry_run: bool,

        /// show a unified configuration diff and imply --dry-run
        #[arg(long)]
        diff: bool,

        /// emit a stable JSON configuration change report
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum FigureSubcommand {
    /// generate bitfield diagrams from JSON files
    Bitfield {
        /// source JSON files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// vertical space
        #[arg(long)]
        vspace: Option<u32>,

        /// horizontal space
        #[arg(long)]
        hspace: Option<u32>,

        /// rectangle lanes
        #[arg(long)]
        lanes: Option<u32>,

        /// overall bitwidth
        #[arg(long)]
        bits: Option<u32>,

        /// font family
        #[arg(long, default_value = "sans-serif")]
        fontfamily: String,

        /// font weight
        #[arg(long, default_value = "normal")]
        fontweight: String,

        /// font size
        #[arg(long, default_value = "14")]
        fontsize: u32,

        /// stroke width
        #[arg(long, default_value = "1.0")]
        strokewidth: f32,

        /// beautify output
        #[arg(long)]
        beautify: bool,

        /// use json5 parser
        #[arg(long)]
        json5: bool,

        /// do not use json5 parser
        #[arg(long)]
        no_json5: bool,

        /// compact mode
        #[arg(long)]
        compact: bool,

        /// horizontal flip
        #[arg(long)]
        hflip: bool,

        /// vertical flip
        #[arg(long)]
        vflip: bool,

        /// trim long bitfield names (character width)
        #[arg(long)]
        trim: Option<f32>,

        /// uneven lanes
        #[arg(long)]
        uneven: bool,

        /// legend item (format: NAME:TYPE, can be used multiple times)
        #[arg(long)]
        legend: Vec<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "svg")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from drawio files
    Drawio {
        /// source drawio files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// drawio executable path
        #[arg(short = 'd', long)]
        drawio: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from graphviz dot files
    Dot {
        /// source dot files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// graphviz dot executable path
        #[arg(short = 'g', long)]
        gradot: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// generate diagrams from plantuml files
    Plantuml {
        /// source plantuml files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// plantuml executable path or jar file path
        #[arg(short = 'p', long)]
        plantuml: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "png")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// export KiCad schematics as publication-ready figures
    Kicad {
        /// KiCad schematic files (*.kicad_sch)
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// kicad-cli executable path
        #[arg(long = "kicad-cli")]
        kicad_cli: Option<String>,

        /// output format (svg or pdf)
        #[arg(short = 'f', long, default_value = "svg")]
        format: String,

        /// export in black and white
        #[arg(short = 'b', long)]
        black_and_white: bool,

        /// omit the KiCad drawing sheet and title block
        #[arg(short = 'e', long)]
        exclude_drawing_sheet: bool,

        /// comma-separated schematic page numbers
        #[arg(long)]
        pages: Option<String>,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// convert images (SVG and other formats)
    Convert {
        /// source image files
        #[arg(required = true, value_hint = ValueHint::FilePath)]
        sources: Vec<String>,

        /// inkscape executable path
        #[arg(short = 'i', long)]
        inkscape: Option<String>,

        /// imagemagick executable path
        #[arg(short = 'm', long)]
        imagemagick: Option<String>,

        /// output format (pdf, png, svg, etc.)
        #[arg(short = 'f', long, default_value = "pdf")]
        format: String,

        /// force regenerate even if output exists
        #[arg(short = 'F', long)]
        force: bool,

        /// output directory
        #[arg(short = 'o', long)]
        output: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        CheckSubcommand, Commands, ConfigScope, ConfigSubcommand, ConfigWriteScope,
        ConvertSubcommand, LibSubcommand, OmniCli, PluginSubcommand,
    };
    use crate::doctype::DocumentFormat;
    use clap::Parser;

    #[test]
    fn latex_backend_is_an_override_only_when_explicitly_provided() {
        let defaults = OmniCli::try_parse_from(["omnidoc", "build"]).expect("default build CLI");
        let Commands::Build { latex_backend, .. } = defaults.command else {
            panic!("expected build command");
        };
        assert_eq!(latex_backend, None);

        let explicit = OmniCli::try_parse_from(["omnidoc", "build", "--latex-backend", "engine"])
            .expect("explicit backend");
        let Commands::Build { latex_backend, .. } = explicit.command else {
            panic!("expected build command");
        };
        assert_eq!(latex_backend.as_deref(), Some("engine"));
    }

    #[test]
    fn new_supports_inferred_titles_and_direct_template_selection() {
        let cli = OmniCli::try_parse_from([
            "omnidoc", "new", "guide", "--type", "ctex-md", "--format", "markdown",
        ])
        .expect("new command");
        let Commands::New {
            path,
            title,
            doctype,
            format,
            defaults,
            dry_run,
            no_commit,
            json,
            ..
        } = cli.command
        else {
            panic!("expected new command");
        };
        assert_eq!(path, "guide");
        assert_eq!(title, None);
        assert_eq!(doctype.as_deref(), Some("ctex-md"));
        assert_eq!(format, Some(DocumentFormat::Markdown));
        assert!(!defaults);
        assert!(!dry_run);
        assert!(!no_commit);
        assert!(!json);

        let preview = OmniCli::try_parse_from([
            "omnidoc",
            "new",
            "guide",
            "--type",
            "ctex-md",
            "--dry-run",
            "--no-commit",
            "--json",
        ])
        .expect("new preview command");
        assert!(matches!(
            preview.command,
            Commands::New {
                dry_run: true,
                no_commit: true,
                json: true,
                ..
            }
        ));
    }

    #[test]
    fn workflow_groups_and_legacy_conversion_forms_both_parse() {
        let grouped = OmniCli::try_parse_from(["omnidoc", "check", "lint", "--strict", "docs"])
            .expect("grouped check command");
        assert!(matches!(
            grouped.command,
            Commands::Check {
                subcommand: CheckSubcommand::Lint { strict: true, .. }
            }
        ));

        let convert = OmniCli::try_parse_from(["omnidoc", "convert", "html", "README.md"])
            .expect("grouped conversion command");
        assert!(matches!(
            convert.command,
            Commands::Convert {
                subcommand: ConvertSubcommand::Html { .. }
            }
        ));

        let legacy = OmniCli::try_parse_from(["omnidoc", "md2html", "README.md"])
            .expect("legacy conversion command");
        assert!(matches!(legacy.command, Commands::Md2html { .. }));

        let legacy_template = OmniCli::try_parse_from(["omnidoc", "template", "--validate"])
            .expect("legacy template validation command");
        assert!(matches!(
            legacy_template.command,
            Commands::Template { validate: true, .. }
        ));

        let legacy_list =
            OmniCli::try_parse_from(["omnidoc", "list"]).expect("legacy template list command");
        assert!(matches!(legacy_list.command, Commands::List));
    }

    #[test]
    fn shell_completion_accepts_positional_and_legacy_forms() {
        let positional = OmniCli::try_parse_from(["omnidoc", "complete", "zsh"])
            .expect("positional completion shell");
        assert!(matches!(
            positional.command,
            Commands::Complete {
                shell: Some(clap_complete::Shell::Zsh),
                generator: None
            }
        ));

        let legacy = OmniCli::try_parse_from(["omnidoc", "complete", "--generate", "bash"])
            .expect("legacy completion shell");
        assert!(matches!(
            legacy.command,
            Commands::Complete {
                shell: None,
                generator: Some(clap_complete::Shell::Bash)
            }
        ));
    }

    #[test]
    fn project_status_open_and_clean_options_parse() {
        let status = OmniCli::try_parse_from(["omnidoc", "status", "docs", "--json"])
            .expect("status command");
        assert!(matches!(
            status.command,
            Commands::Status {
                path: Some(path),
                json: true
            } if path == "docs"
        ));

        let open =
            OmniCli::try_parse_from(["omnidoc", "open", "docs", "--to", "html", "--print-path"])
                .expect("open command");
        assert!(matches!(
            open.command,
            Commands::Open {
                path: Some(path),
                to: Some(output),
                print_path: true
            } if path == "docs" && output == "html"
        ));

        let clean = OmniCli::try_parse_from([
            "omnidoc",
            "clean",
            "docs",
            "--distclean",
            "--dry-run",
            "--json",
        ])
        .expect("clean command");
        assert!(matches!(
            clean.command,
            Commands::Clean {
                path: Some(path),
                distclean: true,
                dry_run: true,
                json: true
            } if path == "docs"
        ));

        let update = OmniCli::try_parse_from([
            "omnidoc",
            "update",
            "docs",
            "--dry-run",
            "--diff",
            "--no-commit",
            "--json",
        ])
        .expect("update command");
        assert!(matches!(
            update.command,
            Commands::Update {
                path: Some(path),
                dry_run: true,
                diff: true,
                no_commit: true,
                json: true
            } if path == "docs"
        ));

        let init = OmniCli::try_parse_from([
            "omnidoc",
            "init",
            "docs",
            "--type",
            "ctex-md",
            "--no-commit",
            "--dry-run",
            "--diff",
            "--json",
        ])
        .expect("init no-commit command");
        assert!(matches!(
            init.command,
            Commands::Init {
                path: Some(path),
                no_commit: true,
                dry_run: true,
                diff: true,
                json: true,
                ..
            } if path == "docs"
        ));
    }

    #[test]
    fn grouped_and_legacy_config_forms_parse() {
        let show = OmniCli::try_parse_from([
            "omnidoc", "config", "show", "docs", "--scope", "project", "--json",
        ])
        .expect("config show");
        assert!(matches!(
            show.command,
            Commands::Config {
                subcommand: Some(ConfigSubcommand::Show {
                    path: Some(path),
                    scope: ConfigScope::Project,
                    json: true,
                }),
                ..
            } if path == "docs"
        ));

        let get =
            OmniCli::try_parse_from(["omnidoc", "config", "get", "target"]).expect("config get");
        assert!(matches!(
            get.command,
            Commands::Config {
                subcommand: Some(ConfigSubcommand::Get { key, .. }),
                ..
            } if key == "target"
        ));

        let set = OmniCli::try_parse_from([
            "omnidoc",
            "config",
            "set",
            "build.outputs",
            "[\"pdf\", \"html\"]",
            "docs",
            "--scope",
            "project",
            "--dry-run",
            "--diff",
            "--json",
        ])
        .expect("config set");
        assert!(matches!(
            set.command,
            Commands::Config {
                subcommand: Some(ConfigSubcommand::Set {
                    key,
                    path: Some(path),
                    scope: ConfigWriteScope::Project,
                    dry_run: true,
                    diff: true,
                    json: true,
                    ..
                }),
                ..
            } if key == "build.outputs" && path == "docs"
        ));

        let unset = OmniCli::try_parse_from([
            "omnidoc",
            "config",
            "unset",
            "tools.pandoc",
            "--scope",
            "global",
        ])
        .expect("config unset");
        assert!(matches!(
            unset.command,
            Commands::Config {
                subcommand: Some(ConfigSubcommand::Unset {
                    key,
                    scope: ConfigWriteScope::Global,
                    ..
                }),
                ..
            } if key == "tools.pandoc"
        ));

        let legacy = OmniCli::try_parse_from(["omnidoc", "config", "--authors", "Docs Team"])
            .expect("legacy config");
        assert!(matches!(
            legacy.command,
            Commands::Config {
                subcommand: None,
                authors: Some(author),
                ..
            } if author == "Docs Team"
        ));
    }

    #[test]
    fn grouped_and_legacy_library_and_plugin_forms_parse() {
        let library = OmniCli::try_parse_from(["omnidoc", "lib", "verify", "--json"])
            .expect("grouped library verification");
        assert!(matches!(
            library.command,
            Commands::Lib {
                subcommand: Some(LibSubcommand::Verify { json: true }),
                ..
            }
        ));

        let legacy_library = OmniCli::try_parse_from(["omnidoc", "lib", "--verify", "--json"])
            .expect("legacy library verification");
        assert!(matches!(
            legacy_library.command,
            Commands::Lib {
                subcommand: None,
                verify: true,
                json: true,
                ..
            }
        ));

        let plugins = OmniCli::try_parse_from(["omnidoc", "plugin", "validate", "docs", "--json"])
            .expect("grouped plugin validation");
        assert!(matches!(
            plugins.command,
            Commands::Plugin {
                subcommand: Some(PluginSubcommand::Validate {
                    path: Some(path),
                    json: true,
                }),
                ..
            } if path == "docs"
        ));

        let examples = OmniCli::try_parse_from(["omnidoc", "plugin", "examples", "docs", "--json"])
            .expect("plugin example discovery");
        assert!(matches!(
            examples.command,
            Commands::Plugin {
                subcommand: Some(PluginSubcommand::Examples {
                    path: Some(path),
                    json: true,
                }),
                ..
            } if path == "docs"
        ));

        let add = OmniCli::try_parse_from([
            "omnidoc",
            "plugin",
            "add",
            "quality-gate",
            "docs",
            "--dry-run",
            "--json",
        ])
        .expect("plugin example installation");
        assert!(matches!(
            add.command,
            Commands::Plugin {
                subcommand: Some(PluginSubcommand::Add {
                    preset,
                    path: Some(path),
                    dry_run: true,
                    json: true,
                }),
                ..
            } if preset == "quality-gate" && path == "docs"
        ));

        let legacy_plugins =
            OmniCli::try_parse_from(["omnidoc", "plugin", "docs", "--validate", "--json"])
                .expect("legacy plugin validation");
        assert!(matches!(
            legacy_plugins.command,
            Commands::Plugin {
                subcommand: None,
                path: Some(path),
                validate: true,
                json: true,
            } if path == "docs"
        ));
    }
}
