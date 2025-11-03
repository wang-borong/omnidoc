use super::project::Doc;
use super::templates::{generate_template, TemplateDocType};
use crate::constants::git as git_constants;
use crate::constants::{dirs, lang, paths, paths_internal};
use crate::doc::templates::generator::try_generate_dynamic;
use crate::doctype::DocumentType;
use crate::doctype::DocumentTypeRegistry;
use crate::error::{OmniDocError, Result};
use crate::fs;
use crate::git::{git_add, git_commit, git_init, is_git_repo};
use console::style;
use std::path::Path;
use walkdir::WalkDir;

impl<'a> Doc<'a> {
    /// Create a new project
    pub fn create_project(&self) -> Result<()> {
        self.init_project(false)
    }

    /// Initialize the project
    pub fn init_project(&self, update: bool) -> Result<()> {
        let md = Path::new(paths::MD_DIR);
        // Just use the last texinput path
        let texinput = self.envs["texinputs"]
            .clone()
            .unwrap_or_else(|| lang::LATEX.to_string());
        let texinput = texinput.strip_suffix(":").unwrap_or(&texinput);
        let texinputs = texinput.split(":").collect::<Vec<&str>>();
        let last_texinput = texinputs.last().unwrap_or(&lang::LATEX);
        let tex = Path::new(&last_texinput);

        // Generate entry file if not updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        // Initialize git repo if needed
        if !is_git_repo(".") {
            git_init(".", true).map_err(|e| OmniDocError::Git(e))?;
        }

        // Create directories based on document type
        let doctype_chk = String::from(&self.doctype);
        let dirs_to_create = vec![
            dirs::DAC_DIR,
            dirs::DRAWIO_DIR,
            dirs::FIGURES_DIR,
            dirs::BIBLIO_DIR,
        ];

        if !md.exists() && doctype_chk.ends_with(lang::MARKDOWN) {
            fs::create_dir(&md)?;
        }
        if !tex.exists() && doctype_chk.ends_with(lang::LATEX) {
            fs::create_dir(&tex)?;
        }

        for dir in dirs_to_create {
            let dir_path = Path::new(dir);
            if !dir_path.exists()
                && (!doctype_chk.contains("resume") || doctype_chk.contains("moderncv"))
            {
                fs::create_dir(&dir_path)?;
            }
        }

        if !Path::new(paths::FIGURE_DIR).exists() {
            fs::create_dir(paths::FIGURE_DIR)?;
        }

        // Move existing .md and .tex files to appropriate directories
        for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let fext = path.extension().and_then(|s| s.to_str());
            let fstem = path.file_stem().and_then(|s| s.to_str());

            if path.is_file()
                && (fext == Some(lang::MARKDOWN) || fext == Some(lang::LATEX))
                && path.parent() == Some(Path::new(paths_internal::CURRENT_DIR))
            {
                let file_name = match path.file_name() {
                    Some(f) => f,
                    None => return Err(OmniDocError::Other("file_name not found".to_string())),
                };

                if fstem == Some(crate::constants::file_names::MAIN)
                    || fstem == Some(crate::constants::file_names::README)
                {
                    continue;
                }

                let destination = if fext == Some(lang::MARKDOWN) {
                    md.join(file_name)
                } else {
                    tex.join(file_name)
                };

                fs::rename(path, destination)?;
            }
        }

        // Create figure README
        let fig_readme = "**Figures in this directory are third-party,\n\
                          and may be used in the document project!\n\
                          If you have no idea where the figures come from,\n\
                          you must not remove them.**";
        Doc::gen_file(&fig_readme, paths::FIGURE_README)?;

        // Copy gitignore from lib
        fs::copy_from_lib(paths_internal::REPO_GITIGNORE, paths::GITIGNORE)
            .map_err(|e| OmniDocError::Io(e))?;

        // Git add and commit
        git_add(".", &["*"], false).map_err(|e| OmniDocError::Git(e))?;

        let cmsg = if update {
            git_constants::UPDATE_COMMIT_MSG
        } else {
            git_constants::INITIAL_COMMIT_MSG
        };
        git_commit(".", cmsg).map_err(|e| OmniDocError::Git(e))?;

        if update {
            println!(
                "{} {} '{}'",
                style("✔").green().bold(),
                style("Project files updated at").green().bold(),
                &self.path.display()
            );
        } else {
            println!(
                "{} {} '{}'",
                style("✔").green().bold(),
                style("Project initialized at").green().bold(),
                &self.path.display()
            );
        }
        Ok(())
    }

    /// Update the project
    pub fn update_project(&mut self) -> Result<()> {
        let update_files = vec![
            paths::FIGURE_README,
            paths::MAKEFILE,
            paths::LATEXMKRC,
            paths::GITIGNORE,
        ];

        for uf in update_files {
            if Path::new(uf).exists() {
                fs::remove_file(uf)?;
            }
        }

        self.init_project(true)
    }

    fn create_entry(&self, title: &str, doctype_str: &str) -> Result<()> {
        // First try dynamic template by key
        if let Some((content, is_markdown, file_name)) =
            try_generate_dynamic(doctype_str, title, &self.author)
        {
            if Path::new(&file_name).exists() {
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
        if Path::new(file_name).exists() {
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
