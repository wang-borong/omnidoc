pub mod generator;
pub mod types;

pub use generator::{generate_latex_template, generate_markdown_template, generate_template};
pub use types::TemplateDocType;
