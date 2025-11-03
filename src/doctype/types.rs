use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DocumentType {
    CtexMd,
    EbookMd,
    EnoteMd,
    CtexartTex,
    CtexrepTex,
    CtexbookTex,
    EbookTex,
    EnoteTex,
    CtartTex,
    CtrepTex,
    CtbookTex,
    ResumeNgTex,
    ModerncvTex,
}

impl DocumentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentType::CtexMd => "ctex-md",
            DocumentType::EbookMd => "ebook-md",
            DocumentType::EnoteMd => "enote-md",
            DocumentType::CtexartTex => "ctexart-tex",
            DocumentType::CtexrepTex => "ctexrep-tex",
            DocumentType::CtexbookTex => "ctexbook-tex",
            DocumentType::EbookTex => "ebook-tex",
            DocumentType::EnoteTex => "enote-tex",
            DocumentType::CtartTex => "ctart-tex",
            DocumentType::CtrepTex => "ctrep-tex",
            DocumentType::CtbookTex => "ctbook-tex",
            DocumentType::ResumeNgTex => "resume-ng-tex",
            DocumentType::ModerncvTex => "moderncv-tex",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DocumentType::CtexMd => "ctex class based markdown document writing system",
            DocumentType::EbookMd => "elegantbook class based markdown document writing system",
            DocumentType::EnoteMd => "elegantnote class based markdown document writing system",
            DocumentType::CtexartTex => "raw ctexart document type",
            DocumentType::CtexrepTex => "raw ctexrep document type",
            DocumentType::CtexbookTex => "raw ctexbook document type",
            DocumentType::EbookTex => "elegantbook class based latex document writing system",
            DocumentType::EnoteTex => "elegantnote class based latex document writing system",
            DocumentType::CtartTex => "ctart class based latex document writing system",
            DocumentType::CtrepTex => "ctrep class based latex document writing system",
            DocumentType::CtbookTex => "ctbook class based latex document writing system",
            DocumentType::ResumeNgTex => "resume-ng document type",
            DocumentType::ModerncvTex => "moderncv document type",
        }
    }

    pub fn file_extension(&self) -> &'static str {
        if self.as_str().ends_with("-md") {
            "md"
        } else {
            "tex"
        }
    }

    pub fn file_name(&self) -> &'static str {
        if self.as_str().ends_with("-md") {
            "main.md"
        } else {
            "main.tex"
        }
    }

    pub fn is_resume_type(&self) -> bool {
        matches!(self, DocumentType::ResumeNgTex | DocumentType::ModerncvTex)
    }
}
