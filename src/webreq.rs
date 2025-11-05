use crate::error::{OmniDocError, Result};
use reqwest::blocking::get;
use std::fs::File;
use std::io::copy;
use std::io::Cursor;
use std::path::Path;

pub fn https_download<P>(url: &str, file: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let response = get(url).map_err(|_e| OmniDocError::HttpError {
        status: 0,
        url: url.to_string(),
    })?;

    if !response.status().is_success() {
        return Err(OmniDocError::HttpError {
            status: response.status().as_u16(),
            url: url.to_string(),
        });
    }

    let mut file = File::create(file).map_err(|e| OmniDocError::Io(e))?;
    let mut content = Cursor::new(response.bytes().map_err(|_e| OmniDocError::HttpError {
        status: 0,
        url: url.to_string(),
    })?);

    copy(&mut content, &mut file).map_err(|e| OmniDocError::Io(e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_download() {
        let r = https_download(
            "https://raw.githubusercontent.com/wang-borong/embedded-knowledge/main/Makefile",
            "downloadfile",
        );
        assert_eq!(r.is_ok(), true);
    }
}
