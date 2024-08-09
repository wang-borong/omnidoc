// File and directory operation

use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::WalkDir;
use std::env;

use super::fs;
use super::cmd::do_cmd;
use super::git::{git_init, git_add, git_commit};

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

        if !pathbuf.exists() {
            let _ = fs::create_dir(&pathbuf);
        }
        // NOTE: We changed to the project directory,
        // the all document operations are in its directory.
        let _ = env::set_current_dir(&pathbuf);

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

    fn gen_file(cont: &str, target: &str) -> Result<(), std::io::Error>
    {
        let target_file = PathBuf::from(target);

        let mut target_fh = fs::File::create(&target_file)?;
        target_fh.write_all(cont.as_bytes())?;

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

        match git_init(".", true) {
            Ok(_) => {},
            Err(e) => eprintln!("git init project failed {}", e),
        }

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
                //println!("Moved: {} to {}", path.display(), fext.unwrap());
            }
        }

        Doc::gen_file(include_str!("../assets/docfig-readme.md"), "figure/README.md")?;
        Doc::gen_file(include_str!("../assets/Makefile"), "Makefile")?;
        Doc::gen_file(include_str!("../assets/gitignore"), ".gitignore")?;
        Doc::gen_file(include_str!("../assets/latexmkrc"), ".latexmkrc")?;

        match git_add(".", &[".gitignore", "Makefile", ".latexmkrc", "figure/README.md"], false) {
            Ok(_) => {},
            Err(e) => eprintln!("git add files failed {}", e),
        }

        match git_commit(".", "Create project") {
            Ok(_) => {},
            Err(e) => eprintln!("git commit failed {}", e),
        }

        println!("omnify project '{}' has been created", projdir.display());

        Ok(())
    }

    pub fn build_project(&self, _o: Option<String>, _b: Option<String>) -> Result<(), std::io::Error> {
        // call make to do default building
        do_cmd("make", &[])?;

        Ok(())
    }

    pub fn clean_project(&self, distclean: bool) -> Result<(), std::io::Error> {
        if distclean {
            // we can just remove output directory in the doc path
            let mut output_dir = self.path.to_path_buf();
            output_dir.push("output");
            fs::remove_dir_all(&output_dir.as_path())?;
        } else {
            // ... call make clean
            do_cmd("make", &["clean"])?;
        }

        Ok(())
    }
}

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

