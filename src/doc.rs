// File and directory operation

use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::WalkDir;
use std::env;
use std::io::Error;

use super::fs;
use super::cmd::do_cmd;
use super::git::{git_init, git_add, git_commit};

#[derive(Debug, PartialEq)]
pub struct Doc {
    title: String,
    path: PathBuf,
    author: String,
    version: String,
    release: String,
    language: String,
    doctype:  String,
    docname:  String,
}

impl Doc {
    pub fn new<P>(title: &str, path: P, author: &str,
        version: &str, release: &str, language: &str,
        doctype: &str, docname: &str) -> Self
        where P: AsRef<Path>
    {
        let pathbuf = PathBuf::new().join(path);

        if !pathbuf.exists() {
            let _ = fs::create_dir_all(&pathbuf);
        }
        // NOTE: We changed to the project directory,
        // the all document operations are in its directory.
        let _ = env::set_current_dir(&pathbuf);

        Self {
            title: String::from(title),
            path: pathbuf,
            author: String::from(author),
            version: String::from(version),
            release: String::from(release),
            language: String::from(language),
            doctype:  String::from(doctype),
            docname:  String::from(docname),
        }
    }

    pub fn create_project(&self) -> Result<(), Error> {
        self.init_project()?;

        Ok(())
    }    

    fn gen_file(cont: &str, target: &str) -> Result<(), Error>
    {
        let target_file = PathBuf::from(target);

        let mut target_fh = fs::File::create(&target_file)?;
        target_fh.write_all(cont.as_bytes())?;

        Ok(())
    }

    pub fn init_project(&self) -> Result<(), Error> {

        let projdir = Path::new(&self.path);
        let md = Path::new("md");
        let tex = Path::new("tex");
        let dac = Path::new("dac");
        let drawio = Path::new("drawio");
        let figure = Path::new("figure");
        let figures = Path::new("figures");
        let biblio = Path::new("biblio");

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
        if !biblio.exists() {
            fs::create_dir(&biblio)?;
        }

        // Walk through the current directory and find .md or .tex files
        for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            let fext = path.extension().and_then(|s| s.to_str());
            let fstem = path.file_stem().and_then(|s| s.to_str());
            if path.is_file() && (fext == Some("md")
                            || fext == Some("tex")) {
                let file_name = path.file_name().unwrap();
                let destination;

                if fstem == Some("main") {
                    continue;
                }

                if fext == Some("md") {
                    destination = md.join(file_name);
                } else {
                    destination = tex.join(file_name);
                }

                // Move the file to the 'md' or 'tex' directory
                fs::rename(path, destination)?;
                //println!("Moved: {} to {}", path.display(), fext.unwrap());
            }
        }

        Doc::gen_file(include_str!("../assets/docfig-readme.md"), "figure/README.md")?;

        let mk_cont = include_str!("../assets/Makefile").to_string();
        let new_mk_cont = mk_cont.replace("TARGET ?= ",
            &format!("TARGET := {}", &self.docname));
        Doc::gen_file(&new_mk_cont, "Makefile")?;

        Doc::gen_file(include_str!("../assets/gitignore"), ".gitignore")?;
        Doc::gen_file(include_str!("../assets/latexmkrc"), ".latexmkrc")?;

        match self.create_entry(&self.title, &self.doctype) {
            Ok(_) => { },
            Err(e) => { eprintln!("create entry failed {}", e) }
        }

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

    fn check_project(&self) -> bool {
        let latexmkrc = Path::new(".latexmkrc");
        let makefile = Path::new("Makefile");

        if latexmkrc.exists() && makefile.exists() {
            true
        } else {
            false
        }
    }

    pub fn build_project(&self, o: Option<String>, _b: Option<String>) -> Result<(), Error> {
        // check if the path is a valid omnify document
        if !self.check_project() {
            return Err(Error::other("not a omnify document path"));
        }

        // create build dir
        match o {
            Some(od) => {
                if !Path::new(&od).exists() {
                    fs::create_dir(&od)?;
                }
                env::set_var("OUTDIR", &od);
            },
            None     => {
                if !Path::new("build").exists() {
                    fs::create_dir("build")?;
                }
                env::set_var("OUTDIR", "build");
            },
        }

        // call make to do default building
        do_cmd("make", &[])?;

        Ok(())
    }

    pub fn clean_project(&self, distclean: bool) -> Result<(), Error> {
        // check if the path is a valid omnify document
        if !self.check_project() {
            return Err(Error::other("not a omnify document path"));
        }

        if distclean {
            do_cmd("make", &["dist-clean"])?;
        } else {
            // ... call make clean
            do_cmd("make", &["clean"])?;
        }

        Ok(())
    }

    fn create_entry(&self, title: &str, doctype: &str) -> Result<(), Error> {

        match doctype {
            "ebook-md" => {
                let cont = entry::make_md(title, &self.author, entry::DocType::EBOOK);
                Doc::gen_file(&cont, "main.md")?;
                match git_add(".", &["main.md"], false) {
                    Ok(_) => {},
                    Err(e) => eprintln!("git add files failed {}", e),
                }
            },
            "enote-md" => {
                let cont = entry::make_md(title, &self.author, entry::DocType::ENOTE);
                Doc::gen_file(&cont, "main.md")?;
                match git_add(".", &["main.md"], false) {
                    Ok(_) => {},
                    Err(e) => eprintln!("git add files failed {}", e),
                }
            },
            _ => {  },
        };

        Ok(())
    }
}

mod entry {
    use indoc::formatdoc;
    use chrono::prelude::*;

    pub enum DocType {
        EBOOK,
        ENOTE,
    }

    pub fn make_md(title: &str, author: &str, dt: DocType) -> String {

        let local: DateTime<Local> = Local::now();
        let date = local.format("%Y/%m/%d").to_string();

        let ebook = r#"documentclass: elegantbook
papersize: a4
classoption:
  - cn
  - chinese
  - fancy
  - onecol
  - device=normal"#;

        let enote = r#"documentclass: elegantnote
papersize: a4
classoption:
  - cn
  - device=normal"#;

        let doctype: &str;
        match dt {
            DocType::EBOOK => doctype = ebook,
            DocType::ENOTE => doctype = enote,
        }
            
        let entry_md = formatdoc!(r#"
---
title: {title}
author:
  - {author}
date:
  - {date}

{doctype}

indent: true
listings: true
numbersections:
  - sectiondepth: 5

#bibliography: cs.bib
#nocite: |
#  @*

csl: computer.csl
#colorlinks: true
graphics: true

toc: true
lof: true
lot: true

header-includes:
  - |
    ```{{=latex}}
    \usepackage{{bookmark}}
    \usepackage{{xr}}
    \usepackage{{lstlangarm}}
    \usepackage{{pdfpages}}

    \usepackage{{utils}}

    \usepackage{{circuitikz}}

    \usepackage{{wrapfig}}
    \usepackage{{enumitem}}
    \setlist[description]{{nosep,labelindent=2em,leftmargin=4em}}

    \usepackage{{bytefield}}
    \lstset{{defaultdialect=[ARM]Assembler}}
    \captionsetup{{font=small}}

    %\cover{{cover}}
    ```

include-before:
  - |
    ```{{=latex}}
    ```
include-after:
  - |
    ```{{=latex}}
    ```
before-body:
  - |
    ```{{=latex}}
    ```
after-body:
  - |
    ```{{=latex}}
    ```
...

```{{.include}}

```

\newpage
# 参考文献"#, title = title, author = author, date = date, doctype = doctype);

        entry_md
    }
}

#[cfg(test)]
mod tests {
}

