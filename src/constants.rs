pub mod paths {
    pub const DEFAULT_BUILD_DIR: &str = "build";
    pub const MAIN_MD: &str = "main.md";
    pub const MAIN_TEX: &str = "main.tex";
    pub const FIGURE_DIR: &str = "figure";
    pub const FIGURE_README: &str = "figure/README.md";
    pub const MD_DIR: &str = "md";
    pub const GITIGNORE: &str = ".gitignore";
    pub const LATEXMKRC: &str = ".latexmkrc";
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

pub mod paths_internal {
    pub const CURRENT_DIR: &str = ".";
    pub const PARENT_DIR: &str = "..";
    pub const PARENT_PARENT_DIR: &str = "../..";
}

pub mod file_names {
    pub const MAIN: &str = "main";
    pub const README: &str = "README";
    pub const PDF_EXTENSION: &str = "pdf";
    pub const HTML_EXTENSION: &str = "html";
}

pub mod commands {
    pub const XDG_OPEN: &str = "xdg-open";
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

pub mod pandoc {
    // Command
    pub const CMD: &str = "pandoc";

    // Common flags
    pub const FLAG_FROM: &str = "-f";
    pub const FLAG_TO: &str = "-t";
    pub const FLAG_FILTER: &str = "-F";
    pub const FLAG_METADATA: &str = "--metadata";
    pub const FLAG_DATA_DIR: &str = "--data-dir";
    pub const FLAG_RESOURCE_PATH: &str = "--resource-path";
    pub const FLAG_STANDALONE: &str = "--standalone";
    pub const FLAG_EMBED_RESOURCES: &str = "--embed-resources";
    pub const FLAG_OUTPUT: &str = "-o";
    pub const FLAG_META_SHORT: &str = "-M";

    // PDF-specific
    pub const FLAG_PDF_ENGINE: &str = "--pdf-engine";
    pub const FLAG_SYNTAX_HIGHLIGHTING: &str = "--syntax-highlighting";
    pub const FLAG_TEMPLATE: &str = "--template";
    pub const FLAG_CSS: &str = "--css";

    // Defaults
    pub const DEFAULT_FROM_PDF: &str = "markdown+east_asian_line_breaks+footnotes";
    pub const DEFAULT_FROM_HTML: &str = "markdown";
    pub const DEFAULT_TO_HTML: &str = "html";
    pub const DEFAULT_ENGINE_LATEX: &str = "xelatex";
    pub const DEFAULT_SYNTAX: &str = "idiomatic";
    pub const DEFAULT_TEMPLATE_LATEX: &str = "pantext.latex";
    pub const DEFAULT_PYTHON: &str = "python3";
    pub const DEFAULT_PLUGIN_CROSSREF: &str = "pandoc-crossref";

    // Library paths (relative under omnidoc lib)
    pub const LIB_PANDOC_DATA: &str = "pandoc/data";
    pub const LIB_PANDOC_FILTERS: &str = "pandoc/data/filters";
    pub const LIB_PANDOC_HEADERS: &str = "pandoc/headers";
    pub const LIB_PANDOC_CSL: &str = "pandoc/csl";
    pub const LIB_PANDOC_CROSSREF_YAML: &str = "pandoc/crossref.yaml"; // PDF fallback
    pub const LIB_PANDOC_CROSSREF_YAML_HTML: &str = "pandoc/data/crossref.yaml"; // HTML fallback
    pub const LIB_PANDOC_CSS_DEFAULT: &str = "pandoc/css/advance-editor.css";

    // Resource path defaults
    pub const RESOURCE_PATH_COMMON_SUFFIX: &str = ":image:images:figure:figures:biblio";
}
