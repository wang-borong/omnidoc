use super::project::Doc;
use super::templates::{
    generate_template, get_gitignore_template, get_latexmkrc_template, TemplateDocType,
};
use crate::constants::git as git_constants;
use crate::constants::{dirs, lang, paths, paths_internal};
use crate::doc::templates::generator::try_generate_dynamic;
use crate::doctype::DocumentType;
use crate::doctype::DocumentTypeRegistry;
use crate::error::{OmniDocError, Result};
use crate::git::{git_add, git_commit, git_init, is_git_repo};
use crate::utils::{error, fs};
use console::style;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

impl<'a> Doc<'a> {
    /// Create a new project
    pub fn create_project(&self) -> Result<()> {
        self.init_project(false)
    }

    /// Initialize the project
    pub fn init_project(&self, update: bool) -> Result<()> {
        // Generate entry file if not updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        // Initialize git repo if needed
        self.initialize_git_repo()?;

        // Setup directory structure
        self.setup_directories()?;

        // Move existing files to appropriate directories
        self.move_existing_files()?;

        // Create template files
        self.create_template_files()?;

        // Commit changes
        self.commit_changes(update)?;

        // Print success message
        self.print_success_message(update)?;

        Ok(())
    }

    /// Initialize git repository if needed
    fn initialize_git_repo(&self) -> Result<()> {
        if !is_git_repo(".") {
            error::git_err(git_init(".", true))?;
        }
        Ok(())
    }

    /// Setup directory structure based on document type
    fn setup_directories(&self) -> Result<()> {
        let md = Path::new(paths::MD_DIR);
        let tex = self.get_tex_input_path();

        let doctype_chk = String::from(&self.doctype);
        let dirs_to_create = vec![
            dirs::DAC_DIR,
            dirs::DRAWIO_DIR,
            dirs::FIGURES_DIR,
            dirs::BIBLIO_DIR,
        ];

        // Create markdown directory if needed
        if !fs::exists(&md) && doctype_chk.ends_with(lang::MARKDOWN) {
            fs::create_dir_all(&md)?;
        }

        // Create LaTeX directory if needed
        if !fs::exists(&tex) && doctype_chk.ends_with(lang::LATEX) {
            fs::create_dir_all(&tex)?;
        }

        // Create common directories
        for dir in dirs_to_create {
            let dir_path = Path::new(dir);
            if !fs::exists(dir_path)
                && (!doctype_chk.contains("resume") || doctype_chk.contains("moderncv"))
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
        PathBuf::from(last_texinput)
    }

    /// Move existing .md and .tex files to appropriate directories
    fn move_existing_files(&self) -> Result<()> {
        let md = Path::new(paths::MD_DIR);
        let tex = self.get_tex_input_path();

        for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let fext = path.extension().and_then(|s| s.to_str());
            let fstem = path.file_stem().and_then(|s| s.to_str());

            // Skip if not a markdown or LaTeX file in root directory
            if !fs::is_file(path)
                || !(fext == Some(lang::MARKDOWN) || fext == Some(lang::LATEX))
                || path.parent() != Some(Path::new(paths_internal::CURRENT_DIR))
            {
                continue;
            }

            // Skip main and README files
            if fstem == Some(crate::constants::file_names::MAIN)
                || fstem == Some(crate::constants::file_names::README)
            {
                continue;
            }

            // Move file to appropriate directory
            let file_name = path
                .file_name()
                .ok_or_else(|| OmniDocError::Other("Could not retrieve file name".to_string()))?;
            let destination = if fext == Some(lang::MARKDOWN) {
                md.join(file_name)
            } else {
                tex.join(file_name)
            };

            crate::utils::fs::rename(path, destination)?;
        }

        Ok(())
    }

    /// Create template files (README, .gitignore, .latexmkrc)
    fn create_template_files(&self) -> Result<()> {
        // Create figure README
        let fig_readme = "**Figures in this directory are third-party,\n\
                          and may be used in the document project!\n\
                          If you have no idea where the figures come from,\n\
                          you must not remove them.**";
        Doc::gen_file(&fig_readme, paths::FIGURE_README)?;

        // Write embedded gitignore template
        let gitignore_content = get_gitignore_template();
        Doc::gen_file(gitignore_content, paths::GITIGNORE)?;

        // Write embedded latexmkrc template
        let latexmkrc_content = get_latexmkrc_template();
        Doc::gen_file(latexmkrc_content, paths::LATEXMKRC)?;

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
        let update_files = vec![paths::FIGURE_README, paths::LATEXMKRC, paths::GITIGNORE];

        for uf in update_files {
            if fs::exists(Path::new(uf)) {
                fs::remove_file(uf)?;
            }
        }

        self.init_project(true)
    }

    fn create_entry(&self, title: &str, doctype_str: &str) -> Result<()> {
        // First try dynamic template by key
        if let Some((content, _is_markdown, file_name)) =
            try_generate_dynamic(doctype_str, title, &self.author)
        {
            if fs::exists(Path::new(&file_name)) {
                return Ok(());
            }
            Doc::gen_file(&content, &file_name)?;
            return Ok(());
        }

        let doctype = DocumentTypeRegistry::from_str(doctype_str)?;

        let template_type = map_document_type_to_template(&doctype)?;
        let is_markdown = doctype.file_extension() == lang::MARKDOWN;
        let file_name = doctype.file_name();

        // Skip if entry file already exists
        if fs::exists(Path::new(file_name)) {
            return Ok(());
        }

        let title_for_template = if doctype.is_resume_type() { "" } else { title };
        let content =
            generate_template(is_markdown, title_for_template, &self.author, template_type);
        Doc::gen_file(&content, file_name)?;

        Ok(())
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
