mod catalog;
pub mod generator;
pub mod repo;
pub mod types;

pub use catalog::{
    list_project_templates, resolve_project_template, ProjectTemplateInfo, TemplateSource,
    DEFAULT_TEMPLATE_KEY,
};
pub use generator::{generate_latex_template, generate_markdown_template, generate_template};
pub use repo::{get_gitignore_template, get_latexmkrc_template};
pub use types::TemplateDocType;
