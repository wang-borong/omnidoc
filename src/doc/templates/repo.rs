/// Get the embedded .gitignore template content
pub fn get_gitignore_template() -> &'static str {
    include_str!("../../../templates/repo/gitignore")
}

/// Get the embedded .latexmkrc template content
pub fn get_latexmkrc_template() -> &'static str {
    include_str!("../../../templates/repo/latexmkrc")
}
