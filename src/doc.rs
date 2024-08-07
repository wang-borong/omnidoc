// File and directory operation

use std::io::Write;
use std::path::{Path, PathBuf};
use std::fmt;
use std::string::String;
use walkdir::WalkDir;
use std::env;
use dirs::config_dir;

use super::fs;
use super::config::read_download_config;
use super::webreq::https_download;
use super::cmd::do_cmd;

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
    pub fn new<P>(name: &str, path: P, author: &str, version: &str, release: &str, language: &str) -> Self
        where P: AsRef<Path>
    {
        let pathbuf = PathBuf::new().join(path).join(name);
        Self {
            name: String::from(name),
            path: pathbuf,
            author: String::from(author),
            version: String::from(version),
            release: String::from(release),
            language: String::from(language),
        }
    }

    pub fn create_project(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.path)?;

        self.init_project()?;

        Ok(())
    }    

    pub fn init_project(&self) -> Result<(), std::io::Error> {

        let projdir = Path::new(&self.path);
        let md = Path::new("md");
        let tex = Path::new("tex");
        let dac = Path::new("dac");
        let drawio = Path::new("drawio");
        let figure = Path::new("figure");
        let figures = Path::new("figures");

        if !projdir.exists() {
            fs::create_dir(&projdir)?;
        }
        env::set_current_dir(&projdir)?;

        if !md.exists() {
            fs::create_dir(&md)?;
        }
        if !tex.exists() {
            fs::create_dir(&tex)?;
        }
        if !dac.exists() {
            fs::create_dir(&dac)?;
        }
        if !drawio.exists() {
            fs::create_dir(&drawio)?;
        }
        if !figure.exists() {
            fs::create_dir(&figure)?;
        }
        if !figures.exists() {
            fs::create_dir(&figures)?;
        }

        // move all markdown
        //fs::rename(from, to);
        // Walk through the current directory and find .md or .tex files
        for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let fext = path.extension().and_then(|s| s.to_str());
            if path.is_file() && (fext == Some("md")
                            || fext == Some("tex")) {
                let file_name = path.file_name().unwrap();
                let destination;

                if fext == Some("md") {
                    destination = md.join(file_name);
                } else {
                    destination = tex.join(file_name);
                }

                // Move the file to the 'md' directory
                fs::rename(path, destination)?;
                println!("Moved: {:?} to {}", path, fext.unwrap());
            }
        }

        let figreadme_str = include_str!("../assets/docfig-readme.md");
        let fr_path = figure.join("README.md");
        let mut figreadme = fs::File::create(&fr_path)?;
        figreadme.write_all(figreadme_str.as_bytes())?;

        let conf_dir = config_dir();
        let conf_file = conf_dir.unwrap().join("omnidoc.toml");
        let conf = read_download_config(&conf_file);
        
        for (url, filename) in &conf.unwrap() {
            match https_download(url, filename) {
                Err(_) => eprintln!("error to download {}", filename),
                Ok(_) => {},
            }
        }

        Ok(())
    }

    pub fn build_project(&self, o: Option<String>, b: Option<String>) -> Result<(), std::io::Error> {
        // call make
        do_cmd("make", &[])?;

        Ok(())
    }

    pub fn clean_project(&self) -> Result<(), std::io::Error> {
        // TODO
        // if distclean {
        // we can just remove output directory in the doc path
        let mut output_dir = self.path.to_path_buf();
        output_dir.push("output");
        fs::remove_dir_all(&output_dir.as_path())?;
        // } else {
        // ... call make clean
        //do_cmd("make", &["clean"])?;
        // }

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
    fn test_doc_init() {
        let mydoc = Doc::new("mydoc", &PathBuf::from("./compiler"), "wbr", "v0.1", "v1.0", "zh_CN");

        let r = mydoc.init_project();
        assert_eq!(r.is_ok(), true);
    }

    #[test]
    fn test_doc_build() {

    }

    #[test]
    fn test_doc_clean() {

    }
}

