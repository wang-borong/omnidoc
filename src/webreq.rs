use std::fs::File;
use std::io::copy;
use std::io::Cursor;
use reqwest::blocking::get;
use std::path::Path;

pub fn https_download<P>(url: &str, file: P) -> Result<(), Box<dyn std::error::Error>>
    where P: AsRef<Path>
{
    // Send the GET request and download the file content
    let response = get(url)?;

    // Ensure the request was successful
    if response.status().is_success() {
        // Create a new file to save the downloaded content
        let mut file = File::create(file)?;
        let mut content = Cursor::new(response.bytes()?);

        // Copy the content to the file
        copy(&mut content, &mut file)?;
    }
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_download() {

        let r = https_download("https://raw.githubusercontent.com/wang-borong/embedded-knowledge/main/Makefile", "downloadfile");
        assert_eq!(r.is_ok(), true);
    }
}
