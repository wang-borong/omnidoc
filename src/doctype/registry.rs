use crate::doctype::types::DocumentType;
use crate::error::{OmniDocError, Result};

pub struct DocumentTypeRegistry;

impl DocumentTypeRegistry {
    pub fn all() -> Vec<DocumentType> {
        vec![
            DocumentType::CtexMd,
            DocumentType::EbookMd,
            DocumentType::EnoteMd,
            DocumentType::CtexartTex,
            DocumentType::CtexrepTex,
            DocumentType::CtexbookTex,
            DocumentType::EbookTex,
            DocumentType::EnoteTex,
            DocumentType::CtartTex,
            DocumentType::CtrepTex,
            DocumentType::CtbookTex,
            DocumentType::ResumeNgTex,
            DocumentType::ModerncvTex,
        ]
    }

    pub fn from_str(s: &str) -> Result<DocumentType> {
        DocumentTypeRegistry::all()
            .into_iter()
            .find(|dt| dt.as_str() == s)
            .ok_or_else(|| OmniDocError::UnsupportedDocumentType(s.to_string()))
    }

    pub fn list_display() -> String {
        DocumentTypeRegistry::all()
            .iter()
            .map(|dt| format!("✅ {}  ({})", dt.as_str(), dt.description()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
