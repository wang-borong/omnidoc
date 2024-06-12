use clap::{Parser, Subcommand};
use std::fmt;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// set project name
    #[arg(short, long)]
    project: String,
    /// set author name
    #[arg(short, long)]
    author: Option<String>,
    /// set document version
    #[arg(short, long)]
    docver: Option<String>,
    /// set release name
    #[arg(short, long)]
    release: Option<String>,
    /// set language
    #[arg(short, long)]
    language: Option<String>,
    /// set suffix
    #[arg(short, long)]
    suffix: Option<String>,

    /// create makefile
    #[arg(long, default_value_t = false)]
    makefile: bool,

    //#[arg(short, long, default_value_t = 1)]
    //count: u8,
}

impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "project: {}\n\
            author: {}\n\
            docver: {}\n\
            release: {}\n\
            language: {}\n\
            suffix: {}\n\
            makefile: {}"
            , self.project
            , self.author.as_ref().unwrap_or(&"None".to_string())
            , self.docver.as_ref().unwrap_or(&"None".to_string())
            , self.release.as_ref().unwrap_or(&"None".to_string())
            , self.language.as_ref().unwrap_or(&"None".to_string())
            , self.suffix.as_ref().unwrap_or(&"None".to_string())
            , self.makefile
        )
    }
}

pub fn cli() {
    let args = Args::parse();

    println!("{:?}", args);
}

