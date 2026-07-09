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

    let content = response.bytes().map_err(|_e| OmniDocError::HttpError {
        status: 0,
        url: url.to_string(),
    })?;

    write_bytes_to_file(content.as_ref(), file)?;

    Ok(())
}

fn write_bytes_to_file<P>(bytes: &[u8], file: P) -> Result<()>
where
    P: AsRef<Path>,
{
    let mut file = File::create(file).map_err(OmniDocError::Io)?;
    let mut content = Cursor::new(bytes);
    copy(&mut content, &mut file).map_err(OmniDocError::Io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omnidoc_{}_{}_{}",
            name,
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn writes_downloaded_bytes_to_file() {
        let output = temp_file_path("download");

        write_bytes_to_file(b"hello omnidoc", &output).expect("write bytes");

        let content = fs::read(&output).expect("read output");
        assert_eq!(content, b"hello omnidoc");

        let _ = fs::remove_file(output);
    }
}
