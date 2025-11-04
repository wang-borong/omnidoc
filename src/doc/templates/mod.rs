pub mod generator;
pub mod repo;
pub mod types;

pub use generator::{generate_latex_template, generate_markdown_template, generate_template};
pub use repo::{get_gitignore_template, get_latexmkrc_template};
pub use types::TemplateDocType;
