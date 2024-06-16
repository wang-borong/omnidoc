// File and directory operation

use std::path::{Path, PathBuf};
use std::error::Error;
use std::fmt;
use std::string::String;

use super::fs;

#[derive(Debug, PartialEq)]
pub struct Doc {
    name: String,
    path: PathBuf,
    author: String,
    version: String,
    release: String,
    language: String,
}

impl Doc {
    pub fn new(name: &str, path: &Path, author: &str, version: &str, release: &str, language: &str) -> Self {
        Self {
            name: String::from(name),
            path: PathBuf::from(path),
            author: String::from(author),
            version: String::from(version),
            release: String::from(release),
            language: String::from(language),
        }
    }

    pub fn create_project(&self) -> Result<(), std::io::Error> {
        let mut project_path = PathBuf::from(&self.path);
        project_path.push(&self.name);

        fs::create_dir_all(project_path.as_path())?;

        Ok(())
    }    

    pub fn build_project(&self) -> Result<(), std::io::Error> {
        Ok(())
    }

    pub fn clean_project(&self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct DocError {
    //source: DocErrorSrc
}

impl fmt::Display for DocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DocError occured!")
    }
}

//impl Error for DocError {
//    fn source(&self) -> Option<&(dyn Error + 'static)> {
//        Some(&self.source)
//    }
//}

//#[derive(Debug)]
//struct DocErrorSrc;
//
//impl fmt::Display for DocErrorSrc {
//    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//        write!(f, "DocErrorSrc occured")
//    }
//}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_struct_new() {
        let mydoc = Doc::new("mydoc", &PathBuf::from("./mydoc"), "wbr", "v0.1", "v1.0", "zh_CN");
        assert_eq!(mydoc, Doc {
            name: String::from("mydoc"),
            path: PathBuf::from("./mydoc"),
            author: String::from("wbr"),
            version: String::from("v0.1"),
            release: String::from("v1.0"),
            language: String::from("zh_CN")
        })
    }

    #[test]
    fn test_doc_create() {
        let mydoc = Doc::new("mydoc", &PathBuf::from("./mydoc"), "wbr", "v0.1", "v1.0", "zh_CN");

        let r = mydoc.create_project();
        assert_eq!(r.is_ok(), true);
    }

    #[test]
    fn test_doc_build() {

    }

    #[test]
    fn test_doc_clean() {

    }
}

