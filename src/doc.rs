// File and directory operation

use dirs::data_local_dir;
use std::collections::HashMap;
use std::env;
use std::io::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::string::String;
use walkdir::WalkDir;

use super::cmd::do_cmd;
use super::fs;
use super::git::{git_add, git_commit, git_init, is_git_repo};

#[derive(Debug, PartialEq)]
pub struct Doc<'a> {
    title: String,
    path: PathBuf,
    author: String,
    doctype: String,
    envs: HashMap<&'a str, Option<String>>,
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

    fn get_docname(&self) -> String {
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

    pub fn create_project(&self) -> Result<(), Error> {
        self.init_project(false)?;

        Ok(())
    }

    fn gen_file(cont: &str, target: &str) -> Result<(), Error> {
        let target_file = PathBuf::from(target);

        let mut target_fh = fs::File::create(&target_file)?;
        target_fh.write_all(cont.as_bytes())?;

        Ok(())
    }

    pub fn init_project(&self, update: bool) -> Result<(), Error> {
        let md = Path::new("md");
        // Just use the last texinput path
        let texinput = self.envs["texinputs"].clone().unwrap_or("tex".to_owned());
        let texinput = texinput.strip_suffix(":").unwrap_or(&texinput);
        let texinputs = texinput.split(":").collect::<Vec<&str>>();
        let last_texinput = match texinputs.last() {
            Some(t) => t,
            None => "tex",
        };
        let tex = Path::new(&last_texinput);

        // we assume the user has the document entry file when updating
        if !update {
            self.create_entry(&self.title, &self.doctype)?;
        }

        if !is_git_repo(".") {
            match git_init(".", true) {
                Ok(_) => {}
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
            if !dir_path.exists()
                && (!doctype_chk.contains("resume") || doctype_chk.contains("moderncv"))
            {
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
                && path.parent() == Some(Path::new("."))
            {
                let file_name = match path.file_name() {
                    Some(f) => f,
                    None => return Err(Error::other("file_name not found")),
                };
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
            Ok(_) => {}
            Err(e) => {
                return Err(Error::other(format!(
                    "Copy gitignore from lib failed ({})",
                    e
                )))
            }
        }

        match git_add(".", &["*"], false) {
            Ok(_) => {}
            Err(e) => return Err(Error::other(format!("Git add files failed ({})", e))),
        }

        let cmsg: &str;

        if update {
            cmsg = "Update project";
        } else {
            cmsg = "Create project";
        }
        match git_commit(".", cmsg) {
            Ok(_) => {}
            Err(e) => return Err(Error::other(format!("Git commit failed ({})", e))),
        }

        println!("{} '{}' success", cmsg, &self.path.display());

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
        let check_pathes = [
            "./main.md",
            "./main.tex",
            "../main.md",
            "../main.tex",
            "../../main.md",
            "../../main.tex",
        ];
        for p in check_pathes {
            if Path::new(p).exists() {
                match Path::new(p).parent() {
                    Some(p) => {
                        if p.to_str().unwrap_or("") != "" {
                            let _ = env::set_current_dir(p);
                        }
                    }
                    None => {}
                }
                return true;
            }
        }
        false
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
            }
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
                }
                None => {}
            }
        }

        let docname = self.get_docname();
        let target = format!("TARGET={}", &docname);

        let mut topmk = data_local_dir().ok_or_else(|| Error::other("data_local_dir not found"))?;
        topmk.push("omnidoc/tool/top.mk");
        if verbose {
            do_cmd(
                "make",
                &[
                    "-f",
                    topmk.to_str().unwrap_or("topmk not found"),
                    &target,
                    "V=1",
                ],
                false,
            )
        } else {
            do_cmd(
                "make",
                &[
                    "-f",
                    topmk.to_str().unwrap_or("topmk not found"),
                    &target,
                ],
                false,
            )
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
            }
            None => {}
        }

        for env_key in vec!["texinputs", "bibinputs", "texmfhome"] {
            let env_val = &self.envs[env_key];
            match env_val {
                Some(env_val) => {
                    env::set_var(env_key.to_uppercase(), &env_val);
                }
                None => {}
            }
        }

        let docname = self.get_docname();
        let target = format!("TARGET={}", &docname);

        let mut topmk = data_local_dir().ok_or_else(|| Error::other("data_local_dir not found"))?;
        topmk.push("omnidoc/tool/top.mk");

        if distclean {
            do_cmd(
                "make",
                &[
                    "-f",
                    topmk.to_str().unwrap_or("topmk not found"),
                    &target,
                    "dist-clean",
                ],
                false,
            )
        } else {
            do_cmd(
                "make",
                &[
                    "-f",
                    topmk.to_str().unwrap_or("topmk not found"),
                    &target,
                    "clean",
                ],
                false,
            )
        }
    }

    pub fn open_doc(&self) -> Result<(), Error> {
        // check if the path is a valid omnidoc project
        if !Doc::is_omnidoc_project() {
            return Err(Error::other("Not a omnified document path"));
        }

        let conf_o = &self.envs["outdir"];
        let outdir = match conf_o {
            Some(conf_o) => conf_o,
            None => "build",
        };

        let docname = self.get_docname();
        let doc_path_str = format!("{}/{}.pdf", outdir, &docname);

        let doc_path = Path::new(&doc_path_str);
        if !doc_path.exists() {
            return Err(Error::other(format!("The '{}' do not exist", doc_path_str)));
        }

        do_cmd(
            "xdg-open",
            &[doc_path.to_str().unwrap_or("doc_path not found")],
            true,
        )
    }

    fn gen_entry_file(
        &self,
        lang: u8,
        title: &str,
        doctype: entry::DocType,
        file: &str,
    ) -> Result<(), Error> {
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
            "ebook-md" => self.gen_entry_file(1, title, entry::DocType::EBOOK, "main.md")?,
            "enote-md" => self.gen_entry_file(1, title, entry::DocType::ENOTE, "main.md")?,
            "ctexart-tex" => self.gen_entry_file(2, title, entry::DocType::CTEXART, "main.tex")?,
            "ctexrep-tex" => self.gen_entry_file(2, title, entry::DocType::CTEXREP, "main.tex")?,
            "ctexbook-tex" => {
                self.gen_entry_file(2, title, entry::DocType::CTEXBOOK, "main.tex")?
            }
            "ebook-tex" => self.gen_entry_file(2, title, entry::DocType::EBOOK, "main.tex")?,
            "enote-tex" => self.gen_entry_file(2, title, entry::DocType::ENOTE, "main.tex")?,
            "ctbook-tex" => self.gen_entry_file(2, title, entry::DocType::CTBOOK, "main.tex")?,
            "ctart-tex" => self.gen_entry_file(2, title, entry::DocType::CTART, "main.tex")?,
            "ctrep-tex" => self.gen_entry_file(2, title, entry::DocType::CTREP, "main.tex")?,
            "resume-ng-tex" => self.gen_entry_file(2, "", entry::DocType::RESUMENG, "main.tex")?,
            "moderncv-tex" => self.gen_entry_file(2, "", entry::DocType::MODERNCV, "main.tex")?,
            _ => return Err(Error::other(format!("Unsupported doctype '{}'", doctype))),
        };

        Ok(())
    }
}

mod entry {
    use chrono::prelude::*;
    use indoc::formatdoc;

    #[derive(PartialEq, PartialOrd)]
    pub enum DocType {
        EBOOK,
        ENOTE,
        CTEXART,
        CTEXREP,
        CTEXBOOK,
        CTBOOK,
        CTART,
        CTREP,
        RESUMENG,
        MODERNCV,
    }

    pub fn make_tex(title: &str, author: &str, dt: DocType) -> String {
        let local: DateTime<Local> = Local::now();
        let date = local.format("%Y/%m/%d").to_string();

        let doclass: &str;
        let mut frontmatter: &str = r"";
        let mut mainmatter: &str = r"";

        match dt {
            DocType::CTEXBOOK | DocType::EBOOK | DocType::CTBOOK => {
                if dt == DocType::EBOOK {
                    doclass = "\\documentclass[\
                        lang=cn,\n\
                        scheme=chinese,\n\
                        mode=fancy,\n\
                        device=normal,\n\
                        ]{elegantbook}\n\
                        \\usepackage{elegant}";
                } else if dt == DocType::CTBOOK {
                    doclass = r#"\documentclass{ctbook}
\usepackage{ctbook}"#;
                } else {
                    doclass = r"\documentclass{ctexbook}";
                }
                frontmatter = r"\frontmatter % only for book";
                mainmatter = r"\mainmatter % only for book";
            }
            DocType::CTEXART | DocType::ENOTE | DocType::CTART => {
                if dt == DocType::EBOOK {
                    doclass = "\\documentclass[\
                        lang=cn,\n\
                        device=normal,\n\
                        ]{elegantnote}\n\
                        \\usepackage{elegant}";
                } else if dt == DocType::CTART {
                    doclass = r#"\documentclass{ctart}
\usepackage{ctart}"#;
                } else {
                    doclass = r"\documentclass{ctexart}";
                }
            }
            DocType::CTREP | DocType::CTEXREP => {
                if dt == DocType::CTREP {
                    doclass = "\\documentclass{ctrep}\n\
                        \\usepackage{ctrep}";
                } else {
                    doclass = "\\documentclass{ctexrep}";
                }
            }
            DocType::RESUMENG => {
                doclass = "\\documentclass{resume-ng}\n\
                    \\usepackage{resume-ng}";
            }
            DocType::MODERNCV => {
                doclass = "";
            }
        }

        if dt == DocType::MODERNCV {
            formatdoc!(
                r#"\documentclass[11pt, a4paper]{{moderncv}}

% optional argument are 'blue' (default), 'orange',
% 'red', 'green', 'grey' and 'roman'
% (for roman fonts, instead of sans serif fonts).
\moderncvtheme[blue]{{classic}}

\usepackage[fontset=adobe]{{moderncv}}

% If you want to change the width of the column with the dates,
% uncomment the below line.
%\setlength{{\hintscolumnwidth}}{{3cm}}

% Only for the classic theme. If you want to change the
% width of your name placeholder (to leave more space
% for your address details), uncomment below line.
%\AtBeginDocument{{\setlength{{\maketitlenamewidth}}{{6cm}}}}

% Required when changes are made to page layout lengths
\AtBeginDocument{{\recomputelengths}}

% personal data
\CNname{{{author}}}
%\title{{}} % Your applied position, like 嵌入式高级工程师
%\address{{}}{{}} % Optional, remove the line if not wanted
%\born{{}} % Optional, remove the line if not wanted
%\mobile{{}} % Optional, remove the line if not wanted
%\email{{}} % Optional, remove the line if not wanted
%\homepage{{}} % Optional, remove the line if not wanted
%\social[github]{{GitHub: }}
%\extrainfo{{%
%  微信：
%}}

% '80pt' is the height the picture must be resized to and 'picture'
% is the name of the picture file;
% It's optional, remove the line if not wanted.
%\photo[80pt]{{avatar.png}}
%\quote{{}} % Optional, remove the line if not wanted

% uncomment to suppress automatic page numbering for CVs longer than one page.
%\nopagenumbers{{}}

\newcommand*{{\cvcont}}[2][.25em]{{%
  \cvitem[#1]{{}}{{\begin{{minipage}}[t]{{\listitemcolumnwidth}}#2\end{{minipage}}}}}}

\begin{{document}}

% Input your resume tex file
%\input{{}}

\end{{document}}"#,
                author = author
            )
        } else if dt < DocType::RESUMENG {
            formatdoc!(
                r#"{doclass}

%\addbibresource{{}}

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

\end{{document}}"#,
                doclass = doclass,
                title = title,
                author = author,
                date = date
            )
        } else {
            formatdoc!(
                r#"{doclass}

\ResumeName{{{author}}}

\begin{{document}}

% Input your resume tex file
% \input{{}}

\end{{document}}"#,
                author = author
            )
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
            }
            DocType::ENOTE => {
                doctype = enote;
                latex_header = eheader;
            }
            _ => {
                doctype = r"";
                latex_header = r"";
            }
        }

        let entry_md = formatdoc!(
            r#"
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

#biblatex: true
#biblatexoptions:
#  - backend=biber
#  - citestyle=numeric-comp
#  - bibstyle=numeric
#bibliography:
#  - biblio/cseebook.bib
#nocite-ids:
#  - \*
#biblio-title: 参考文献
#csl: computer.csl

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
"#,
            title = title,
            author = author,
            date = date,
            doctype = doctype,
            latex_header = latex_header
        );

        entry_md
    }
}

#[cfg(test)]
mod tests {}
