use super::project::Doc;
use super::templates::{
    generate_template, get_gitignore_template, get_latexmkrc_template, resolve_project_template,
    ProjectTemplateInfo, TemplateDocType,
};
use crate::constants::git as git_constants;
use crate::constants::{dirs, lang, paths};
use crate::doc::templates::generator::try_generate_dynamic;
use crate::doctype::{DocumentFormat, DocumentType, DocumentTypeRegistry};
use crate::error::{OmniDocError, Result};
use crate::git::{
    git_commit, git_has_commits, git_init, git_stage_all, git_worktree_changes, is_git_repo,
};
use crate::utils::{error, fs};
use console::style;
use serde::Serialize;
use similar::TextDiff;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
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

    pub(crate) fn init_project_with_options(
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
        self.initialize_git_repo()?;

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
    fn initialize_git_repo(&self) -> Result<()> {
        if !is_git_repo(".") {
            error::git_err(git_init(".", false))?;
        }
        Ok(())
    }

    /// Setup directory structure based on document type
    fn setup_directories(&self) -> Result<()> {
        let template = resolve_project_template(&self.doctype)?;
        for directory in self.project_directories(&template) {
            if !fs::exists(&directory) {
                fs::create_dir_all(&directory)?;
            }
        }

        Ok(())
    }

    fn project_directories(&self, template: &ProjectTemplateInfo) -> BTreeSet<PathBuf> {
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
        directories
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
        for (path, content) in self.managed_template_files()? {
            write_managed_file(&path, content)?;
        }

        Ok(())
    }

    fn managed_template_files(&self) -> Result<Vec<(PathBuf, &'static str)>> {
        let template = resolve_project_template(&self.doctype)
            .map_err(|error| OmniDocError::Project(format!("Invalid document type: {error}")))?;
        let mut files = vec![
            (self.path.join(paths::FIGURE_README), FIGURE_README_CONTENT),
            (self.path.join(paths::GITIGNORE), get_gitignore_template()),
        ];
        if template.format == DocumentFormat::Latex {
            files.push((self.path.join(paths::LATEXMKRC), get_latexmkrc_template()));
        }
        Ok(files)
    }

    /// Commit changes to git
    fn commit_changes(&self, update: bool) -> Result<bool> {
        error::git_err(git_stage_all("."))?;
        if error::git_err(git_worktree_changes("."))?.is_empty() {
            return Ok(false);
        }

        let cmsg = if update {
            git_constants::UPDATE_COMMIT_MSG
        } else {
            git_constants::INITIAL_COMMIT_MSG
        };
        error::git_err(git_commit(".", cmsg))?;

        Ok(true)
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
        let file_moves = self.planned_file_moves()?;
        self.init_project_with_planned_moves(true, commit, print_success, &file_moves)
    }

    pub fn plan_update(
        &self,
        commit: bool,
        include_diff: bool,
    ) -> Result<Vec<ProjectUpdateAction>> {
        let template = resolve_project_template(&self.doctype)
            .map_err(|error| OmniDocError::Project(format!("Invalid document type: {error}")))?;
        let mut actions = Vec::new();
        for (path, content) in self.managed_template_files()? {
            if let Some(change) = managed_file_change(&path, content)? {
                let diff = include_diff
                    .then(|| managed_file_diff(&self.path, &path, content))
                    .transpose()?;
                actions.push(managed_update_action(path, change, diff));
            }
        }

        let repository_exists = is_git_repo(&self.path);
        let repository_has_commits = if repository_exists {
            error::git_err(git_has_commits(&self.path))?
        } else {
            false
        };
        if !repository_exists {
            actions.push(update_action(
                "initialize_git",
                self.path.join(".git"),
                None,
            ));
        }

        let file_moves = self.planned_file_moves()?;
        let mut directories = self.project_directories(&template);
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

        let has_commit_changes = !repository_exists
            || !repository_has_commits
            || actions
                .iter()
                .any(|action| matches!(action.operation.as_str(), "refresh_file" | "move_file"));
        if commit && has_commit_changes {
            actions.push(update_action("commit", self.path.clone(), None));
        }
        Ok(actions)
    }

    /// Describe a new project without touching the target path.
    pub fn plan_new(&self, commit: bool) -> Result<Vec<ProjectUpdateAction>> {
        let template = resolve_project_template(&self.doctype)
            .map_err(|error| OmniDocError::Project(format!("Invalid document type: {error}")))?;
        let entry = self.path.join(&template.file_name);
        let mut directories = self.project_directories(&template);
        if let Some(parent) = entry
            .parent()
            .filter(|parent| *parent != self.path.as_path())
        {
            directories.insert(parent.to_path_buf());
        }

        let mut actions = vec![
            update_action("create_directory", self.path.clone(), None),
            create_file_action(self.path.join(paths::PROJECT_CONFIG)),
            create_file_action(entry),
            update_action("initialize_git", self.path.join(".git"), None),
        ];
        actions.extend(
            directories
                .into_iter()
                .map(|directory| update_action("create_directory", directory, None)),
        );
        actions.extend(
            self.managed_template_files()?
                .into_iter()
                .map(|(path, _)| create_file_action(path)),
        );
        if commit {
            actions.push(update_action("commit", self.path.clone(), None));
        }
        Ok(actions)
    }

    fn resolve_project_path(&self, path: &Path) -> PathBuf {
        let resolved = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.path.join(path)
        };
        resolved.components().collect()
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
        change: None,
        diff: None,
    }
}

fn managed_update_action(path: PathBuf, change: &str, diff: Option<String>) -> ProjectUpdateAction {
    ProjectUpdateAction {
        operation: "refresh_file".to_string(),
        path: path.to_string_lossy().to_string(),
        destination: None,
        change: Some(change.to_string()),
        diff,
    }
}

fn create_file_action(path: PathBuf) -> ProjectUpdateAction {
    ProjectUpdateAction {
        operation: "create_file".to_string(),
        path: path.to_string_lossy().to_string(),
        destination: None,
        change: Some("create".to_string()),
        diff: None,
    }
}

fn managed_file_change(path: &Path, content: &str) -> Result<Option<&'static str>> {
    match std::fs::read(path) {
        Ok(existing) if existing == content.as_bytes() => Ok(None),
        Ok(_) => Ok(Some("update")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Some("create")),
        Err(error) => Err(OmniDocError::Io(error)),
    }
}

fn write_managed_file(path: &Path, content: &str) -> Result<bool> {
    if managed_file_change(path, content)?.is_none() {
        return Ok(false);
    }
    fs::atomic_write(path, content.as_bytes())?;
    Ok(true)
}

fn managed_file_diff(project_root: &Path, path: &Path, content: &str) -> Result<String> {
    let (original, old_label) = match std::fs::read(path) {
        Ok(bytes) => (
            String::from_utf8_lossy(&bytes).into_owned(),
            format!("a/{}", portable_relative_path(project_root, path)),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (String::new(), "/dev/null".to_string())
        }
        Err(error) => return Err(OmniDocError::Io(error)),
    };
    let new_label = format!("b/{}", portable_relative_path(project_root, path));
    Ok(TextDiff::from_lines(&original, content)
        .unified_diff()
        .header(&old_label, &new_label)
        .to_string())
}

fn portable_relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
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
