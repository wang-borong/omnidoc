// Re-export the main Doc struct for backward compatibility
mod initializer;
mod opener;
mod project;
pub mod services;
pub mod templates;
pub mod utils;

// Doc methods are implemented in initializer and opener modules
// These re-exports are kept for backward compatibility but may not be directly used
#[allow(unused_imports)]
pub use initializer::*;
#[allow(unused_imports)]
pub use opener::*;
pub use project::Doc;
pub use utils::is_omnidoc_project;

// Re-export template types for backward compatibility
pub use templates::TemplateDocType;

impl<'a> Doc<'a> {
    /// Check if current directory is an omnidoc project
    pub fn is_omnidoc_project() -> bool {
        utils::is_omnidoc_project()
    }
}
