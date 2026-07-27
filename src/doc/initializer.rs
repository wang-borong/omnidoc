use super::project::Doc;
use super::templates::{
    generate_template, get_gitignore_template, get_latexmkrc_template, resolve_project_template,
    TemplateDocType,
};
use crate::constants::git as git_constants;
use crate::constants::{dirs, lang, paths};
use crate::doc::templates::generator::try_generate_dynamic;
use crate::doctype::{DocumentFormat, DocumentType, DocumentTypeRegistry};
use crate::error::{OmniDocError, Result};
use crate::git::{git_add, git_commit, git_init, is_git_repo};
use crate::utils::{error, fs};
use console::style;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const FIGURE_README_CONTENT: &str = "**Figures in this directory are third-party,\n\
                                      and may be used in the document project!\n\
                                      If you have no idea where the figures come from,\n\
                                      you must not remove them.**";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProjectUpdateAction {
    pub operation: String,
    pub path: String,
    pub destination: Option<String>,
}

impl<'a> Doc<'a> {
    /// Create a new project
    pub fn create_project(&self) -> Result<()> {
        self.init_project(false)
    }

    /// Initialize the project
    pub fn init_project(&self, update: bool) -> Result<()> {
        self.init_project_with_options(update, true, true)
    }

    fn init_project_with_options(
        &self,
        update: bool,
        commit: bool,
        print_success: bool,
    ) -> Result<()> {
        let file_moves = self.planned_file_moves()?;
        self.init_project_with_planned_moves(update, commit, print_success, &file_moves)
    }

    fn init_project_with_planned_moves(
        &self,
        update: bool,
        commit: bool,
        print_success: bool,
        file_moves: &[(PathBuf, PathBuf)],
    ) -> Result<()> {
        // Generate entry file if not updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        // Initialize git repo if needed
        self.initialize_git_repo(commit)?;

        // Setup directory structure
        self.setup_directories()?;

        // Move existing files to appropriate directories
        self.move_existing_files(file_moves)?;

        // Create template files
        self.create_template_files()?;

        // Commit changes
        if commit {
            self.commit_changes(update)?;
        }

        // Print success message
        if print_success {
            self.print_success_message(update)?;
        }

        Ok(())
    }

    /// Initialize git repository if needed
    fn initialize_git_repo(&self, create_initial_commit: bool) -> Result<()> {
        if !is_git_repo(".") {
            error::git_err(git_init(".", create_initial_commit))?;
        }
        Ok(())
    }

    /// Setup directory structure based on document type
    fn setup_directories(&self) -> Result<()> {
        let md = Path::new(paths::MD_DIR);
        let tex = self.get_tex_input_path();

        let template = resolve_project_template(&self.doctype)?;
        let dirs_to_create = vec![
            dirs::DAC_DIR,
            dirs::DRAWIO_DIR,
            dirs::FIGURES_DIR,
            dirs::BIBLIO_DIR,
        ];

        // Create markdown directory if needed
        if !fs::exists(md) && template.format == DocumentFormat::Markdown {
            fs::create_dir_all(md)?;
        }

        // Create LaTeX directory if needed
        if !fs::exists(&tex) && template.format == DocumentFormat::Latex {
            fs::create_dir_all(&tex)?;
        }

        // Create common directories
        for dir in dirs_to_create {
            let dir_path = Path::new(dir);
            if !fs::exists(dir_path)
                && (!template.key.contains("resume") || template.key.contains("moderncv"))
            {
                fs::create_dir_all(dir_path)?;
            }
        }

        // Create figure directory if needed
        if !fs::exists(Path::new(paths::FIGURE_DIR)) {
            fs::create_dir_all(Path::new(paths::FIGURE_DIR))?;
        }

        Ok(())
    }

    /// Get the LaTeX input path from envs
    fn get_tex_input_path(&self) -> PathBuf {
        let texinput = self.envs["texinputs"]
            .clone()
            .unwrap_or_else(|| lang::LATEX.to_string());
        let texinput = texinput.strip_suffix(":").unwrap_or(&texinput);
        let texinputs: Vec<&str> = texinput.split(":").collect();
        let last_texinput = texinputs.last().unwrap_or(&lang::LATEX);
        Path::new(last_texinput).components().collect()
    }

    /// Move existing .md and .tex files to appropriate directories
    fn move_existing_files(&self, file_moves: &[(PathBuf, PathBuf)]) -> Result<()> {
        for (source, destination) in file_moves {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            crate::utils::fs::rename(source, destination)?;
        }

        Ok(())
    }

    fn planned_file_moves(&self) -> Result<Vec<(PathBuf, PathBuf)>> {
        let markdown_dir = self.path.join(paths::MD_DIR);
        let latex_dir = self.resolve_project_path(&self.get_tex_input_path());
        let mut file_moves = Vec::new();

        for entry in std::fs::read_dir(&self.path).map_err(OmniDocError::Io)? {
            let entry = entry.map_err(OmniDocError::Io)?;
            let source = entry.path();
            if !source.is_file() {
                continue;
            }

            let extension = source.extension().and_then(|extension| extension.to_str());
            if !matches!(extension, Some(lang::MARKDOWN | lang::LATEX)) {
                continue;
            }

            let stem = source.file_stem().and_then(|stem| stem.to_str());
            if matches!(
                stem,
                Some(crate::constants::file_names::MAIN | crate::constants::file_names::README)
            ) {
                continue;
            }

            let file_name = source.file_name().ok_or_else(|| {
                OmniDocError::Project(format!("Source file has no name: {}", source.display()))
            })?;
            let destination = if extension == Some(lang::MARKDOWN) {
                markdown_dir.join(file_name)
            } else {
                latex_dir.join(file_name)
            };
            if destination.exists() {
                return Err(OmniDocError::Project(format!(
                    "Cannot move '{}' to '{}': the destination already exists. Move or rename one of the files, then run `omnidoc update` again.",
                    source.display(),
                    destination.display()
                )));
            }
            file_moves.push((source, destination));
        }

        file_moves.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(file_moves)
    }

    /// Create template files (README, .gitignore, .latexmkrc)
    fn create_template_files(&self) -> Result<()> {
        // Create figure README
        Doc::gen_file(FIGURE_README_CONTENT, paths::FIGURE_README)?;

        // Write embedded gitignore template
        let gitignore_content = get_gitignore_template();
        Doc::gen_file(gitignore_content, paths::GITIGNORE)?;

        // Write embedded latexmkrc template only for LaTeX document types
        let template = resolve_project_template(&self.doctype)
            .map_err(|e| OmniDocError::Project(format!("Invalid document type: {}", e)))?;
        if template.format == DocumentFormat::Latex {
            let latexmkrc_content = get_latexmkrc_template();
            Doc::gen_file(latexmkrc_content, paths::LATEXMKRC)?;
        }

        Ok(())
    }

    /// Commit changes to git
    fn commit_changes(&self, update: bool) -> Result<()> {
        error::git_err(git_add(".", &["*"], false))?;

        let cmsg = if update {
            git_constants::UPDATE_COMMIT_MSG
        } else {
            git_constants::INITIAL_COMMIT_MSG
        };
        error::git_err(git_commit(".", cmsg))?;

        Ok(())
    }

    /// Print success message
    fn print_success_message(&self, update: bool) -> Result<()> {
        let message = if update {
            ("Project updated in", &self.path.display())
        } else {
            ("Project initialized in", &self.path.display())
        };

        println!(
            "{} {} '{}'",
            style("✔").green().bold(),
            style(message.0).green().bold(),
            message.1
        );

        Ok(())
    }

    /// Update the project
    pub fn update_project(&mut self) -> Result<()> {
        self.update_project_with_options(true, true)
    }

    pub fn update_project_with_options(&mut self, commit: bool, print_success: bool) -> Result<()> {
        // Validate all source moves before refreshing any managed file so an
        // update can never partially mutate a project because of a collision.
        let file_moves = self.planned_file_moves()?;
        let mut update_files = vec![paths::FIGURE_README, paths::GITIGNORE];

        // Only update .latexmkrc for LaTeX document types
        let template = resolve_project_template(&self.doctype)
            .map_err(|e| OmniDocError::Project(format!("Invalid document type: {}", e)))?;
        if template.format == DocumentFormat::Latex {
            update_files.push(paths::LATEXMKRC);
        }

        for uf in update_files {
            if fs::exists(Path::new(uf)) {
                fs::remove_file(uf)?;
            }
        }

        self.init_project_with_planned_moves(true, commit, print_success, &file_moves)
    }

    pub fn plan_update(&self, commit: bool) -> Result<Vec<ProjectUpdateAction>> {
        let template = resolve_project_template(&self.doctype)
            .map_err(|error| OmniDocError::Project(format!("Invalid document type: {error}")))?;
        let mut actions = Vec::new();
        for file in [paths::FIGURE_README, paths::GITIGNORE] {
            actions.push(update_action("refresh_file", self.path.join(file), None));
        }
        if template.format == DocumentFormat::Latex {
            actions.push(update_action(
                "refresh_file",
                self.path.join(paths::LATEXMKRC),
                None,
            ));
        }

        if !is_git_repo(&self.path) {
            actions.push(update_action(
                "initialize_git",
                self.path.join(".git"),
                None,
            ));
        }

        let file_moves = self.planned_file_moves()?;
        let mut directories = BTreeSet::new();
        if !template.key.contains("resume") || template.key.contains("moderncv") {
            directories.insert(self.path.join(dirs::DAC_DIR));
            directories.insert(self.path.join(dirs::DRAWIO_DIR));
            directories.insert(self.path.join(dirs::FIGURES_DIR));
            directories.insert(self.path.join(dirs::BIBLIO_DIR));
        }
        directories.insert(self.path.join(paths::FIGURE_DIR));
        match template.format {
            DocumentFormat::Markdown => {
                directories.insert(self.path.join(paths::MD_DIR));
            }
            DocumentFormat::Latex => {
                directories.insert(self.resolve_project_path(&self.get_tex_input_path()));
            }
        }
        for (_, destination) in &file_moves {
            if let Some(parent) = destination.parent() {
                directories.insert(parent.to_path_buf());
            }
        }
        for directory in directories {
            if !directory.exists() {
                actions.push(update_action("create_directory", directory, None));
            }
        }

        for (source, destination) in file_moves {
            actions.push(update_action("move_file", source, Some(destination)));
        }

        if commit {
            actions.push(update_action("commit", self.path.clone(), None));
        }
        Ok(actions)
    }

    fn resolve_project_path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path.join(path)
        }
    }

    fn create_entry(&self, title: &str, doctype_str: &str) -> Result<()> {
        match DocumentTypeRegistry::parse(doctype_str) {
            Ok(doctype) => {
                let template_type = map_document_type_to_template(&doctype)?;
                let is_markdown = doctype.file_extension() == lang::MARKDOWN;
                let file_name = doctype.file_name();
                if fs::exists(Path::new(file_name)) {
                    return Ok(());
                }

                let title_for_template = if doctype.is_resume_type() { "" } else { title };
                let content =
                    generate_template(is_markdown, title_for_template, &self.author, template_type);
                Doc::gen_file(&content, file_name)
            }
            Err(builtin_error) => {
                let Some((content, _is_markdown, file_name)) =
                    try_generate_dynamic(doctype_str, title, &self.author)
                else {
                    return Err(builtin_error);
                };
                let file_path = Path::new(&file_name);
                if fs::exists(file_path) {
                    return Ok(());
                }
                if let Some(parent) = file_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    fs::create_dir_all(parent)?;
                }
                Doc::gen_file(&content, &file_name)
            }
        }
    }
}

fn update_action(
    operation: &str,
    path: PathBuf,
    destination: Option<PathBuf>,
) -> ProjectUpdateAction {
    ProjectUpdateAction {
        operation: operation.to_string(),
        path: path.to_string_lossy().to_string(),
        destination: destination.map(|path| path.to_string_lossy().to_string()),
    }
}

fn map_document_type_to_template(dt: &DocumentType) -> Result<TemplateDocType> {
    use TemplateDocType::*;
    match dt {
        DocumentType::CtexMd => Ok(CTEXMD),
        DocumentType::EbookMd | DocumentType::EbookTex => Ok(EBOOK),
        DocumentType::EnoteMd | DocumentType::EnoteTex => Ok(ENOTE),
        DocumentType::CtexartTex => Ok(CTEXART),
        DocumentType::CtexrepTex => Ok(CTEXREP),
        DocumentType::CtexbookTex => Ok(CTEXBOOK),
        DocumentType::CtartTex => Ok(CTART),
        DocumentType::CtrepTex => Ok(CTREP),
        DocumentType::CtbookTex => Ok(CTBOOK),
        DocumentType::ResumeNgTex => Ok(RESUMENG),
        DocumentType::ModerncvTex => Ok(MODERNCV),
    }
}
