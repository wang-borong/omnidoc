// File and directory operation

use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::WalkDir;
use std::env;
use std::io::Error;
use std::collections::HashMap;
use dirs::data_local_dir;

use super::fs;
use super::cmd::do_cmd;
use super::git::{git_init, git_add, git_commit, is_git_repo};

#[derive(Debug, PartialEq)]
pub struct Doc<'a> {
    title: String,
    path: PathBuf,
    author: String,
    doctype:  String,
    docname:  String,
    envs: HashMap<&'a str, Option<String>>,
}

impl<'a> Doc<'a> {
    pub fn new<P>(title: &str, path: P, author: &str, doctype: &str,
            envs: HashMap<&'a str, Option<String>>) -> Self
        where P: AsRef<Path>
    {
        let doc_path = PathBuf::new().join(path);
        let cur_dir = env::current_dir().unwrap();
        let doc_path_clone = doc_path.clone();
        let docname = if doc_path != PathBuf::from(".") {
            doc_path_clone.file_name().unwrap().to_str().unwrap_or("unknown")
        } else {
            cur_dir.file_name().unwrap().to_str().unwrap_or("unknown")
        };

        Self {
            title: String::from(title),
            path: doc_path,
            author: String::from(author),
            doctype:  String::from(doctype),
            docname:  String::from(docname),
            envs,
        }
    }

    pub fn create_project(&self) -> Result<(), Error> {
        self.init_project(false)?;

        Ok(())
    }    

    fn gen_file(cont: &str, target: &str) -> Result<(), Error>
    {
        let target_file = PathBuf::from(target);

        let mut target_fh = fs::File::create(&target_file)?;
        target_fh.write_all(cont.as_bytes())?;

        Ok(())
    }

    pub fn init_project(&self, update: bool) -> Result<(), Error> {

        let projdir = Path::new(&self.path);
        let md = Path::new("md");
        // Just use the last texinput path
        let texinput = &self.envs["texinputs"].clone().unwrap_or("tex".to_owned());
        let texinput = texinput.strip_suffix(":").unwrap_or(&texinput);
        let texinputs = texinput.split(":").collect::<Vec<&str>>();
        let last_texinput = texinputs.last().unwrap();
        let tex = Path::new(&last_texinput);

        // we assume the user has the document entry file when updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        if !is_git_repo(&projdir) {
            match git_init(".", true) {
                Ok(_) => {},
                Err(e) => return Err(Error::other(format!("Git init project failed ({})", e))),
            }
        }

        let doctype_chk = String::from(&self.doctype);
        let dirs = vec!["dac", "drawio", "figures", "biblio"];

        if !md.exists() && doctype_chk.ends_with("md") {
            fs::create_dir(&md)?;
        }
        if !tex.exists() && doctype_chk.ends_with("tex") {
            fs::create_dir(&tex)?;
        }
        for dir in dirs {
            let dir_path = Path::new(dir);
            if !dir_path.exists() && !doctype_chk.contains("resume") {
                fs::create_dir(&dir_path)?;
            }
        }
        if !Path::new("figure").exists() {
            fs::create_dir("figure")?;
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

        let fig_readme = "**Figures in this directory are third-party,\n\
                          and may be used in the document project!\n\
                          If you have no idea where the figures come from,\n\
                          you must not remove them.**";
        Doc::gen_file(&fig_readme, "figure/README.md")?;

        match fs::copy_from_lib("repo/gitignore", ".gitignore") {
            Ok(_) => { },
            Err(e) => return Err(Error::other(format!("Copy gitignore from lib failed ({})", e))),
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

    pub fn update_project(&mut self) -> Result<(), Error> {
        let update_files = vec!["figure/README.md", "Makefile", ".latexmkrc", ".gitignore"];

        for uf in update_files {
            if Path::new(uf).exists() {
                fs::remove_file(uf)?;
            }
        }

        self.init_project(true)?;

        Ok(())
    }

    pub fn is_omnidoc_project() -> bool {
        let main_md = Path::new("main.md");
        let main_tex = Path::new("main.tex");

        if main_md.exists() || main_tex.exists() {
            true
        } else {
            false
        }
    }

    pub fn build_project(&self, verbose: bool) -> Result<(), Error> {
        // check if the path is a valid omnify document
        if !Doc::is_omnidoc_project() {
            return Err(Error::other("Not a omnified document path"));
        }

        // create build dir
        let conf_o = &self.envs["outdir"];
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

        for env_key in vec!["texinputs", "bibinputs", "texmfhome"] {
            let env_val = &self.envs[env_key];
            match env_val {
                Some(env_val) => {
                    env::set_var(env_key.to_uppercase(), &env_val);
                },
                None => { },
            }
        }

        let target = format!("TARGET={}", &self.docname);

        let mut topmk = data_local_dir().unwrap();
        topmk.push("omnidoc/tool/top.mk");
        if verbose {
            do_cmd("make", &["-f", &topmk.to_str().unwrap(), &target, "V=1"], false)
        } else {
            do_cmd("make", &["-f", &topmk.to_str().unwrap(), &target], false)
        }
    }

    pub fn clean_project(&self, distclean: bool) -> Result<(), Error> {
        // check if the path is a valid omnidoc project
        if !Doc::is_omnidoc_project() {
            return Err(Error::other("Not a omnified document path"));
        }

        let conf_o = &self.envs["outdir"];
        match conf_o {
            Some(conf_o) => {
                env::set_var("OUTDIR", &conf_o);
            },
            None => { },
        }

        for env_key in vec!["texinputs", "bibinputs", "texmfhome"] {
            let env_val = &self.envs[env_key];
            match env_val {
                Some(env_val) => {
                    env::set_var(env_key.to_uppercase(), &env_val);
                },
                None => { },
            }
        }

        let target = format!("TARGET={}", &self.docname);

        let mut topmk = data_local_dir().unwrap();
        topmk.push("omnidoc/tool/top.mk");

        if distclean {
            do_cmd("make", &["-f", &topmk.to_str().unwrap(), &target, "dist-clean"], false)
        } else {
            do_cmd("make", &["-f", &topmk.to_str().unwrap(), &target, "clean"], false)
        }
    }

    pub fn open_doc(&self) -> Result<(), Error> {
        let conf_o = &self.envs["outdir"];
        let outdir = match conf_o {
            Some(conf_o) => conf_o,
            None => "build",
        };

        let doc_path_str = format!("{}/{}.pdf", outdir, &self.docname);

        let project_path = Path::new(&self.path);
        let doc_path = project_path.join(&doc_path_str);

        if !doc_path.exists() {
            return Err(Error::other(format!("The '{}' do not exist", doc_path_str)));
        }

        do_cmd("xdg-open", &[&doc_path.to_str().unwrap()], true)
    }

    fn gen_entry_file(&self, lang: u8, title: &str, doctype: entry::DocType,
        file: &str) -> Result<(), Error> {
        // if the entry file already exists, skip to generate a new one.
        if Path::new(file).exists() {
            return Ok(());
        }

        let cont = match lang {
            1 => entry::make_md(title, &self.author, doctype),
            2 => entry::make_tex(title, &self.author, doctype),
            _ => return Err(Error::other("Unsupported lang")),
        };

        Doc::gen_file(&cont, file)?;

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
            "myrep-tex"     => self.gen_entry_file(2, title, entry::DocType::MYREPORT, "main.tex")?,
            "resume-ng-tex" => self.gen_entry_file(2, "", entry::DocType::MYRESUME, "main.tex")?,
            _ => { return Err(Error::other(format!("Unsupported doctype '{}'", doctype))) },
        };

        Ok(())
    }
}

mod entry {
    use indoc::formatdoc;
    use chrono::prelude::*;

    #[derive(PartialEq, PartialOrd)]
    pub enum DocType {
        EBOOK,
        ENOTE,
        MYBOOK,
        MYART,
        MYREPORT,
        MYRESUME,
    }

    pub fn make_tex(title: &str, author: &str, dt: DocType) -> String {

        let local: DateTime<Local> = Local::now();
        let date = local.format("%Y/%m/%d").to_string();

        let doclass: &str;
        let mut frontmatter: &str = r"";
        let mut mainmatter: &str = r"";

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
            },
            DocType::MYREPORT => {
                doclass = "\\documentclass{ctrep}\n\
                    \\usepackage{myart}";
            },
            DocType::MYRESUME => {
                doclass = "\\documentclass{resume-ng}\n\
                    \\usepackage{myresume-ng}";
            },
            //_ => {
            //    doclass = r"\usepackage{ctexart}";
            //    frontmatter = r"";
            //    mainmatter = r"";
            //},
        }

        if dt < DocType::MYRESUME {
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
        } else {
            formatdoc!(r#"{doclass}

\ResumeName{{{author}}}

\begin{{document}}

% Input your resume tex file
% \input{{}}

\end{{document}}"#, author = author)
        }
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

        let eheader = r#"
    \usepackage{elegant}
    \usepackage{bookmark}
    \usepackage{xr}
    \usepackage{lstlangarm}
    \usepackage{pdfpages}

    \usepackage{utils}

    \usepackage{circuitikz}

    \usepackage{wrapfig}
    \usepackage{enumitem}
    \setlist[description]{nosep,labelindent=2em,leftmargin=4em}

    \usepackage{bytefield}
    \lstset{defaultdialect=[ARM]Assembler}
    \captionsetup{font=small}

    %\cover{cover}"#;

        let doctype: &str;
        let latex_header: &str;
        match dt {
            DocType::EBOOK => {
                doctype = ebook;
                latex_header = eheader;
            },
            DocType::ENOTE => {
                doctype = enote;
                latex_header = eheader;
            },
            _ => {
                doctype = r"";
                latex_header = r"";
            },
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
    {latex_header}
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
# 参考文献"#, title = title, author = author,
            date = date, doctype = doctype, latex_header = latex_header);

        entry_md
    }
}

#[cfg(test)]
mod tests {
}

