// Re-export the main Doc struct for backward compatibility
mod builder;
mod cleaner;
mod initializer;
mod opener;
mod project;
mod templates;
mod utils;

pub use builder::*;
pub use cleaner::*;
pub use initializer::*;
pub use opener::*;
pub use project::Doc;

// Re-export template types for backward compatibility
pub use templates::TemplateDocType;

impl<'a> Doc<'a> {
    /// Check if current directory is an omnidoc project
    pub fn is_omnidoc_project() -> bool {
        utils::is_omnidoc_project()
    }
}
