pub mod paths {
    pub const DEFAULT_BUILD_DIR: &str = "build";
    pub const MAIN_MD: &str = "main.md";
    pub const MAIN_TEX: &str = "main.tex";
    pub const FIGURE_DIR: &str = "figure";
    pub const FIGURE_README: &str = "figure/README.md";
    pub const MD_DIR: &str = "md";
    pub const GITIGNORE: &str = ".gitignore";
    pub const LATEXMKRC: &str = ".latexmkrc";
    pub const MAKEFILE: &str = "Makefile";
}

pub mod dirs {
    pub const DAC_DIR: &str = "dac";
    pub const DRAWIO_DIR: &str = "drawio";
    pub const FIGURES_DIR: &str = "figures";
    pub const BIBLIO_DIR: &str = "biblio";
}

pub mod git {
    pub const INITIAL_COMMIT_MSG: &str = "Create project";
    pub const UPDATE_COMMIT_MSG: &str = "Update project";
}

pub mod lang {
    pub const MARKDOWN: &str = "md";
    pub const LATEX: &str = "tex";
}

pub mod build {
    pub const TARGET_PREFIX: &str = "TARGET=";
    pub const MAKE_VERBOSE_FLAG: &str = "V=1";
    pub const CLEAN_TARGET: &str = "clean";
    pub const DISTCLEAN_TARGET: &str = "dist-clean";
}

pub mod paths_internal {
    pub const OMNIDOC_TOOL_TOP_MK: &str = "omnidoc/tool/top.mk";
    pub const REPO_GITIGNORE: &str = "repo/gitignore";
    pub const REPO_LATEXMKRC: &str = "repo/latexmkrc";
    pub const CURRENT_DIR: &str = ".";
    pub const PARENT_DIR: &str = "..";
    pub const PARENT_PARENT_DIR: &str = "../..";
}

pub mod file_names {
    pub const MAIN: &str = "main";
    pub const README: &str = "README";
    pub const PDF_EXTENSION: &str = "pdf";
}

pub mod commands {
    pub const XDG_OPEN: &str = "xdg-open";
    pub const MAKE: &str = "make";
}

pub mod git_refs {
    pub const ORIGIN: &str = "origin";
    pub const MAIN_BRANCH: &str = "main";
    pub const HEAD: &str = "HEAD";
    pub const FETCH_HEAD: &str = "FETCH_HEAD";
    pub const REFS_HEADS_PREFIX: &str = "refs/heads/";
}

pub mod git_commits {
    pub const INITIAL_COMMIT_MSG: &str = "Initial commit";
}

pub mod config {
    pub const SECTION_AUTHOR: &str = "[author]\n";
    pub const SECTION_LIB: &str = "[lib]\n";
    pub const SECTION_ENV: &str = "[env]\n";
    pub const NAME_KEY: &str = "name = \"{}\"\n";
    pub const PATH_KEY: &str = "path = \"{}\"\n";
    pub const OUTDIR_KEY: &str = "outdir = \"{}\"\n";
    pub const OUTDIR_DEFAULT: &str = "outdir = \"build\"\n";
    pub const TEXMFHOME_KEY: &str = "texmfhome = \"{}\"\n";
    pub const BIBINPUTS_KEY: &str = "bibinputs = \"{}\"\n";
    pub const TEXINPUTS_KEY: &str = "texinputs = \"{}\"\n";
    pub const PATH_SEPARATOR: &str = "/:";
    pub const OMNIDOC_CONFIG_FILE: &str = "omnidoc.toml";
    pub const CONFIG_DIR: &str = ".config";
    pub const UNKNOWN_AUTHOR: &str = "unknown";
}
