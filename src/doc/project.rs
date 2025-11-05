use crate::error::Result;
use crate::utils::fs;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

/// Document project structure
#[derive(Debug, PartialEq)]
pub struct Doc<'a> {
    pub(super) title: String,
    pub(super) path: PathBuf,
    pub(super) author: String,
    pub(super) doctype: String,
    pub(super) envs: HashMap<&'a str, Option<String>>,
}

impl<'a> Doc<'a> {
    pub fn new(
        title: &str,
        path: &str,
        author: &str,
        doctype: &str,
        envs: HashMap<&'a str, Option<String>>,
    ) -> Self {
        Self {
            title: String::from(title),
            path: PathBuf::from(path),
            author: String::from(author),
            doctype: String::from(doctype),
            envs,
        }
    }

    pub(super) fn get_docname(&self) -> String {
        let cur_dir = match env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return "unknown".to_string(),
        };
        let docname = match cur_dir.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => "unknown",
        };
        String::from(docname)
    }

    pub(super) fn gen_file(cont: &str, target: &str) -> Result<()> {
        fs::write(target, cont.as_bytes())?;
        Ok(())
    }
}
