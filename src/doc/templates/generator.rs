use super::types::TemplateDocType;
use crate::config::ConfigParser;
use chrono::prelude::*;
use std::env;
use tera::{Context, Tera};

fn build_builtin_tera() -> Tera {
    let mut tera = Tera::default();
    let _ = tera.add_raw_template(
        "latex/default.tex",
        include_str!("../../../templates/latex/default.tex"),
    );
    let _ = tera.add_raw_template(
        "latex/resume_ng.tex",
        include_str!("../../../templates/latex/resume_ng.tex"),
    );
    let _ = tera.add_raw_template(
        "latex/moderncv.tex",
        include_str!("../../../templates/latex/moderncv.tex"),
    );
    let _ = tera.add_raw_template(
        "markdown/default.md",
        include_str!("../../../templates/markdown/default.md"),
    );
    tera
}

fn build_external_tera() -> Option<Tera> {
    if let Ok(dir) = env::var("OMNIDOC_TEMPLATE_DIR") {
        let pattern = format!("{}/**/*", dir);
        match Tera::new(&pattern) {
            Ok(t) => Some(t),
            Err(_) => None,
        }
    } else {
        // Try to read from config if available
        if let Ok(cp) = ConfigParser::default() {
            if let Some(dir) = cp.get_template_dir() {
                let pattern = format!("{}/**/*", dir);
                if let Ok(t) = Tera::new(&pattern) {
                    return Some(t);
                }
            }
        }
        None
    }
}
/// Generate LaTeX template content
pub fn generate_latex_template(title: &str, author: &str, dt: TemplateDocType) -> String {
    let local: DateTime<Local> = Local::now();
    let date = local.format("%Y/%m/%d").to_string();

    // Map doc class and sections
    let (doclass, frontmatter, mainmatter) = match dt {
        TemplateDocType::CTEXBOOK | TemplateDocType::EBOOK | TemplateDocType::CTBOOK => {
            let dc = if dt == TemplateDocType::EBOOK {
                "\\documentclass[\n    lang=cn,\n    scheme=chinese,\n    mode=fancy,\n    device=normal,\n]{{elegantbook}}\n\\usepackage{{elegant}}"
            } else if dt == TemplateDocType::CTBOOK {
                "\\documentclass{ctbook}\n\\usepackage{ctbook}"
            } else {
                "\\documentclass{ctexbook}"
            };
            (dc, "\\frontmatter", "\\mainmatter")
        }
        TemplateDocType::CTEXART
        | TemplateDocType::ENOTE
        | TemplateDocType::CTART
        | TemplateDocType::CTEXMD => {
            let dc = if dt == TemplateDocType::ENOTE {
                "\\documentclass[\n    lang=cn,\n    device=normal,\n]{{elegantnote}}\n\\usepackage{{elegant}}"
            } else if dt == TemplateDocType::CTART {
                "\\documentclass{ctart}\n\\usepackage{ctart}"
            } else {
                "\\documentclass{ctexart}"
            };
            (dc, "", "")
        }
        TemplateDocType::CTREP | TemplateDocType::CTEXREP => {
            let dc = if dt == TemplateDocType::CTREP {
                "\\documentclass{ctrep}\n\\usepackage{ctrep}"
            } else {
                "\\documentclass{ctexrep}"
            };
            (dc, "", "")
        }
        TemplateDocType::RESUMENG => (
            "\\documentclass{resume-ng}\n\\usepackage{resume-ng}",
            "",
            "",
        ),
        TemplateDocType::MODERNCV => ("", "", ""),
    };

    if dt == TemplateDocType::MODERNCV {
        let mut ctx = Context::new();
        ctx.insert("author", author);
        let template_name = "latex/moderncv.tex";
        if let Some(ext) = build_external_tera() {
            if let Ok(s) = ext.render(template_name, &ctx) {
                return s;
            }
        }
        let mut builtin = build_builtin_tera();
        let template = r#"\documentclass[11pt, a4paper]{moderncv}

% optional argument are 'blue' (default), 'orange',
% 'red', 'green', 'grey' and 'roman'
% (for roman fonts, instead of sans serif fonts).
\moderncvtheme[blue]{classic}

\usepackage[fontset=adobe]{moderncv}

% If you want to change the width of the column with the dates,
% uncomment the below line.
%\setlength{\hintscolumnwidth}{3cm}

% Only for the classic theme. If you want to change the
% width of your name placeholder (to leave more space
% for your address details), uncomment below line.
%\AtBeginDocument{\setlength{\maketitlenamewidth}{6cm}}

% Required when changes are made to page layout lengths
\AtBeginDocument{\recomputelengths}

% personal data
\CNname{ {{ author }} }
%\title{} % Your applied position
%\address{}{}
%\born{}
%\mobile{}
%\email{}
%\homepage{}
%\social[github]{GitHub: }
%\extrainfo{%
%  微信：
%}

%\photo[80pt]{avatar.png}
%\quote{}

%\nopagenumbers{}

\newcommand*{\cvcont}[2][.25em]{
  \cvitem[#1]{}{\begin{minipage}[t]{\listitemcolumnwidth}#2\end{minipage}}}

\begin{document}

%\input{}

\end{document}
"#;
        return Tera::one_off(template, &ctx, false).unwrap_or_default();
    }

    let mut ctx = Context::new();
    ctx.insert("title", title);
    ctx.insert("author", author);
    ctx.insert("date", &date);
    ctx.insert("doclass", doclass);
    ctx.insert("frontmatter", frontmatter);
    ctx.insert("mainmatter", mainmatter);

    if dt < TemplateDocType::RESUMENG {
        let template_name = "latex/default.tex";
        if let Some(ext) = build_external_tera() {
            if let Ok(s) = ext.render(template_name, &ctx) {
                return s;
            }
        }
        let template = r#"{{ doclass }}

%\addbibresource{}

\title{ {{ title }} }
\author{ {{ author }} }
\date{ {{ date }} }

\begin{document}

{{ frontmatter }}
\maketitle

\tableofcontents

{{ mainmatter }}
% \input{}

\clearpage
%\printbibliography[heading=bibintoc, title=参考文献]

\end{document}
"#;
        Tera::one_off(template, &ctx, false).unwrap_or_default()
    } else {
        let template_name = "latex/resume_ng.tex";
        if let Some(ext) = build_external_tera() {
            if let Ok(s) = ext.render(template_name, &ctx) {
                return s;
            }
        }
        let template = r#"{{ doclass }}

\ResumeName{ {{ author }} }

\begin{document}

% \input{}

\end{document}
"#;
        Tera::one_off(template, &ctx, false).unwrap_or_default()
    }
}

/// Generate Markdown template content
pub fn generate_markdown_template(title: &str, author: &str, dt: TemplateDocType) -> String {
    let local: DateTime<Local> = Local::now();
    let date = local.format("%Y/%m/%d").to_string();

    const EHEADER: &str = r#"\usepackage{elegant}
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

    const CTEXHEADER: &str = r#"\usepackage{omni}
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
\usepackage{caption}
\captionsetup{font=small}"#;

    let (doctype, latex_header) = match dt {
        TemplateDocType::EBOOK => (
            r#"documentclass: elegantbook
papersize: a4
classoption:
  - cn
  - chinese
  - fancy
  - onecol
  - device=normal"#,
            EHEADER,
        ),
        TemplateDocType::ENOTE => (
            r#"documentclass: elegantnote
papersize: a4
classoption:
  - cn
  - device=normal"#,
            EHEADER,
        ),
        TemplateDocType::CTEXMD => (
            r#"documentclass: ctexart
papersize: a4
classoption:
  - sub4section
  - fontset=msword"#,
            CTEXHEADER,
        ),
        _ => ("", ""),
    };

    let mut ctx = Context::new();
    ctx.insert("title", title);
    ctx.insert("author", author);
    ctx.insert("date", &date);
    ctx.insert("doctype", doctype);
    ctx.insert("latex_header", latex_header);

    let template_name = "markdown/default.md";
    if let Some(ext) = build_external_tera() {
        if let Ok(s) = ext.render(template_name, &ctx) {
            return s;
        }
    }
    let template = r#"
---
title: {{ title }}
author:
  - {{ author }}
date:
  - {{ date }}

{{ doctype }}

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
#  - *
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
    ```{=latex}
    {{ latex_header }}
    ```

include-before:
  - |
    ```{=latex}
    ```
include-after:
  - |
    ```{=latex}
    ```
before-body:
  - |
    ```{=latex}
    ```
after-body:
  - |
    ```{=latex}
    ```
...

```{.include}

```
"#;

    Tera::one_off(template, &ctx, false).unwrap_or_default()
}

/// Generate template based on language and document type
pub fn generate_template(
    is_markdown: bool,
    title: &str,
    author: &str,
    dt: TemplateDocType,
) -> String {
    if is_markdown {
        generate_markdown_template(title, author, dt)
    } else {
        generate_latex_template(title, author, dt)
    }
}
