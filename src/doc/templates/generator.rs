use super::types::TemplateDocType;
use chrono::prelude::*;
use indoc::formatdoc;

/// Generate LaTeX template content
pub fn generate_latex_template(title: &str, author: &str, dt: TemplateDocType) -> String {
    let local: DateTime<Local> = Local::now();
    let date = local.format("%Y/%m/%d").to_string();

    let doclass: &str;
    let mut frontmatter: &str = r"";
    let mut mainmatter: &str = r"";

    match dt {
        TemplateDocType::CTEXBOOK | TemplateDocType::EBOOK | TemplateDocType::CTBOOK => {
            if dt == TemplateDocType::EBOOK {
                doclass = "\\documentclass[\
                    lang=cn,\n\
                    scheme=chinese,\n\
                    mode=fancy,\n\
                    device=normal,\n\
                    ]{elegantbook}\n\
                    \\usepackage{elegant}";
            } else if dt == TemplateDocType::CTBOOK {
                doclass = r#"\documentclass{ctbook}
\usepackage{ctbook}"#;
            } else {
                doclass = r"\documentclass{ctexbook}";
            }
            frontmatter = r"\frontmatter % only for book";
            mainmatter = r"\mainmatter % only for book";
        }
        TemplateDocType::CTEXART
        | TemplateDocType::ENOTE
        | TemplateDocType::CTART
        | TemplateDocType::CTEXMD => {
            if dt == TemplateDocType::ENOTE {
                doclass = "\\documentclass[\
                    lang=cn,\n\
                    device=normal,\n\
                    ]{elegantnote}\n\
                    \\usepackage{elegant}";
            } else if dt == TemplateDocType::CTART {
                doclass = r#"\documentclass{ctart}
\usepackage{ctart}"#;
            } else {
                doclass = r"\documentclass{ctexart}";
            }
        }
        TemplateDocType::CTREP | TemplateDocType::CTEXREP => {
            if dt == TemplateDocType::CTREP {
                doclass = "\\documentclass{ctrep}\n\
                    \\usepackage{ctrep}";
            } else {
                doclass = "\\documentclass{ctexrep}";
            }
        }
        TemplateDocType::RESUMENG => {
            doclass = "\\documentclass{resume-ng}\n\
                \\usepackage{resume-ng}";
        }
        TemplateDocType::MODERNCV => {
            doclass = "";
        }
    }

    if dt == TemplateDocType::MODERNCV {
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

\newcommand*{{\cvcont}}[2][.25em]{{
  \cvitem[#1]{{}}{{\begin{{minipage}}[t]{{\listitemcolumnwidth}}#2\end{{minipage}}}}}}

\begin{{document}}

% Input your resume tex file
%\input{{}}

\end{{document}}"#,
            author = author
        )
    } else if dt < TemplateDocType::RESUMENG {
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

/// Generate Markdown template content
pub fn generate_markdown_template(title: &str, author: &str, dt: TemplateDocType) -> String {
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

    let ctexmd = r#"documentclass: ctexart
papersize: a4
classoption:
  - sub4section
  - fontset=msword"#;

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

    let ctexheader = r#"
    \usepackage{omni}
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

    let doctype: &str;
    let latex_header: &str;
    match dt {
        TemplateDocType::EBOOK => {
            doctype = ebook;
            latex_header = eheader;
        }
        TemplateDocType::ENOTE => {
            doctype = enote;
            latex_header = eheader;
        }
        TemplateDocType::CTEXMD => {
            doctype = ctexmd;
            latex_header = ctexheader;
        }
        _ => {
            doctype = r"";
            latex_header = r"";
        }
    }

    formatdoc!(
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
    )
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
