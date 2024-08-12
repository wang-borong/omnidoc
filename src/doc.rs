// File and directory operation

use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::WalkDir;
use std::env;
use std::io::Error;
use regex::Regex;
use sha256::digest;
use std::collections::HashMap;

use super::fs;
use super::cmd::do_cmd;
use super::git::{git_init, git_add, git_commit, git_repo_check};

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

    pub fn create_project(&self, envs: HashMap<&str, Option<String>>) -> Result<(), Error> {
        self.init_project(envs, false)?;

        Ok(())
    }    

    fn gen_file(cont: &str, target: &str) -> Result<(), Error>
    {
        let target_file = PathBuf::from(target);

        let mut target_fh = fs::File::create(&target_file)?;
        target_fh.write_all(cont.as_bytes())?;

        Ok(())
    }

    pub fn init_project(&self, envs: HashMap<&str, Option<String>>, update: bool) -> Result<(), Error> {

        let projdir = Path::new(&self.path);
        let md = Path::new("md");
        // Just use the last texinput path
        let texinput = envs["texinputs"].clone().unwrap_or("tex".to_owned());
        let texinput = texinput.strip_suffix(":").unwrap_or(&texinput);
        let texinputs = texinput.split(":").collect::<Vec<&str>>();
        let last_texinput = texinputs.last().unwrap();
        let tex = Path::new(&last_texinput);
        let dac = Path::new("dac");
        let drawio = Path::new("drawio");
        let figure = Path::new("figure");
        let figures = Path::new("figures");
        let biblio = Path::new("biblio");

        if !git_repo_check(&projdir) {
            match git_init(".", true) {
                Ok(_) => {},
                Err(e) => return Err(Error::other(format!("Git init project failed ({})", e))),
            }
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
            if path.is_file()
                    && (fext == Some("md") || fext == Some("tex"))
                    && path.parent() == Some(Path::new(".")) {
                let file_name = path.file_name().unwrap();
                let destination;

                if fstem == Some("main") || fstem == Some("README") {
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

        // we assume the user has the document entry file when updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        match git_add(".", &["*"], false) {
            Ok(_) => { },
            Err(e) => return Err(Error::other(format!("Git add files failed ({})", e))),
        }

        let cmsg: &str;

        if update {
            cmsg = "Update project";
        } else {
            cmsg = "Create project";
        }
        match git_commit(".", cmsg) {
            Ok(_) => { },
            Err(e) => return Err(Error::other(format!("Git commit failed ({})", e))),
        }

        println!("{} '{}' success", cmsg, projdir.display());

        Ok(())
    }

    fn get_docname(&self) -> Result<String, Error> {
        let re = Regex::new(r"TARGET\s*[\?:]=\s*(.*)").unwrap();
        let contents = match fs::read_to_string("Makefile") {
            Ok(contents) => contents,
            Err(e) => return Err(Error::other(format!("Read Makefile failed ({})", e))),
        };

        let Some(docname) = re.captures(&contents) else {
            return Err(Error::other("Can not match docname in Makefile"));
        };

        Ok(docname[1].to_string())
    }

    fn compare_two_file(&self, f1_cont: &str, f2: &str) -> bool {
        let asset_makefile_sha256 = digest(f1_cont);

        if !Path::new(f2).exists() {
            return false;
        }

        // FIXME: If the file is modified by user on purpose,
        // do we update?
        let cur_makefile_sha256 = match fs::read_to_string(f2) {
            Ok(cont) => digest(cont),
            Err(_) => digest("fake"),
        };
        if asset_makefile_sha256 == cur_makefile_sha256 {
            return true;
        }

        false
    }

    pub fn update_project(&mut self, envs: HashMap<&str, Option<String>>, has_name: bool) -> Result<(), Error> {
        // Check if the project should update
        if self.compare_two_file(include_str!("../assets/Makefile"), "Makefile")
            || self.compare_two_file(include_str!("../assets/latexmkrc"), ".latexmkrc") {
            return Ok(())
        }

        if !has_name {
            self.docname = self.get_docname()?;
        }

        let update_files = vec!["figure/README.md", "Makefile", ".latexmkrc", ".gitignore"];

        for uf in update_files {
            fs::remove_file(uf)?;
        }

        self.init_project(envs, true)?;

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

    pub fn build_project(&self, o: Option<String>, envs: HashMap<&str, Option<String>>, 
                                verbose: bool) -> Result<(), Error> {
        // check if the path is a valid omnify document
        if !self.check_project() {
            return Err(Error::other("Not a omnified document path"));
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
                let conf_o = &envs["outdir"];
                match conf_o {
                    Some(conf_o) => {
                        if !Path::new(&conf_o).exists() {
                            fs::create_dir(&conf_o)?;
                        }
                        env::set_var("OUTDIR", &conf_o);
                    },
                    None => {
                        if !Path::new("build").exists() {
                            fs::create_dir("build")?;
                        }
                    }
                }
            },
        }
        for env_key in vec!["texinputs", "bibinputs", "texmfhome"] {
            let env_val = &envs[env_key];
            match env_val {
                Some(env_val) => {
                    env::set_var(env_key.to_uppercase(), &env_val);
                },
                None => { },
            }
        }

        if verbose {
            do_cmd("make", &["V=1"])?;
        } else {
            do_cmd("make", &[])?;
        }

        Ok(())
    }

    pub fn clean_project(&self, envs: HashMap<&str, Option<String>>, distclean: bool) -> Result<(), Error> {
        // check if the path is a valid omnify document
        if !self.check_project() {
            return Err(Error::other("Not a omnified document path"));
        }

        let conf_o = &envs["outdir"];
        match conf_o {
            Some(conf_o) => {
                env::set_var("OUTDIR", &conf_o);
            },
            None => { },
        }

        for env_key in vec!["texinputs", "bibinputs", "texmfhome"] {
            let env_val = &envs[env_key];
            match env_val {
                Some(env_val) => {
                    env::set_var(env_key.to_uppercase(), &env_val);
                },
                None => { },
            }
        }

        if distclean {
            do_cmd("make", &["dist-clean"])?;
        } else {
            // ... call make clean
            do_cmd("make", &["clean"])?;
        }

        Ok(())
    }

    fn gen_entry_file(&self, lang: u8, title: &str, doctype: entry::DocType,
        file: &str) -> Result<(), Error> {
        // if the entry file already exists, skip to generate a new one.
        if Path::new(file).exists() {
            return Ok(());
        }

        let cont: String;
        match lang {
            1 => cont = entry::make_md(title, &self.author, doctype),
            2 => cont = entry::make_tex(title, &self.author, doctype),
            _ => return Err(Error::other("Unsupported lang")),
        }

        Doc::gen_file(&cont, file)?;
        match git_add(".", &[file], false) {
            Ok(_) => { },
            Err(e) => return Err(Error::other(format!("Git add files failed ({})", e))),
        }

        Ok(())
    }

    fn create_entry(&self, title: &str, doctype: &str) -> Result<(), Error> {

        match doctype {
            "ebook-md"      => self.gen_entry_file(1, title, entry::DocType::EBOOK, "main.md")?,
            "enote-md"      => self.gen_entry_file(1, title, entry::DocType::ENOTE, "main.md")?,
            "ebook-tex"     => self.gen_entry_file(2, title, entry::DocType::EBOOK, "main.tex")?,
            "enote-tex"     => self.gen_entry_file(2, title, entry::DocType::ENOTE, "main.tex")?,
            "mybook-tex"    => self.gen_entry_file(2, title, entry::DocType::MYBOOK, "main.tex")?,
            "myart-tex"     => self.gen_entry_file(2, title, entry::DocType::MYART, "main.tex")?,
            _ => { return Err(Error::other(format!("Unsupported doctype '{}'", doctype))) },
        };

        Ok(())
    }
}

mod entry {
    use indoc::formatdoc;
    use chrono::prelude::*;

    #[derive(PartialEq)]
    pub enum DocType {
        EBOOK,
        ENOTE,
        MYBOOK,
        MYART,
        //MYREPORT,
    }

    pub fn make_tex(title: &str, author: &str, dt: DocType) -> String {

        let local: DateTime<Local> = Local::now();
        let date = local.format("%Y/%m/%d").to_string();

        let doclass: &str;
        let frontmatter: &str;
        let mainmatter: &str;

        match dt {
            DocType::EBOOK | DocType::MYBOOK => {
                if dt == DocType::EBOOK {
                    doclass = r"\documentclass{elegantbook}";
                } else {
                    doclass = r#"\documentclass{ctbook}
\usepackage{mybook}"#;
                }
                frontmatter = r"\frontmatter % only for book";
                mainmatter = r"\mainmatter % only for book";
            },
            DocType::ENOTE | DocType::MYART => {
                if dt == DocType::EBOOK {
                    doclass = r"\documentclass{elegantnote}";
                } else {
                    doclass = r#"\documentclass{ctart}
\usepackage{myart}"#;
                }
                frontmatter = r"";
                mainmatter = r"";

            },
            //_ => {
            //    doclass = r"\usepackage{ctexart}";
            //    frontmatter = r"";
            //    mainmatter = r"";
            //},
        }

        formatdoc!(r#"{doclass}

%\addbibresource{{}}

% 设置标题，作者与时间
\title{{{title}}}
\author{{{author}}}
\date{{{date}}}

\begin{{document}}

{frontmatter}
\maketitle

\tableofcontents

{mainmatter}
% Input your tex files
% \input{{}}

\clearpage
%\printbibliography[heading=bibintoc, title=参考文献]

\end{{document}}"#, doclass = doclass, title = title, author = author, date = date)
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
            _ => doctype = r"",
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

