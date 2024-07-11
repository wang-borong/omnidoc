use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

//
// [[download]]
// url = ""
// filename = ""
//
#[derive(Deserialize)]
struct DownloadConfig {
    url: String,
    filename: String,
}

#[derive(Deserialize)]
struct Config {
    download: Vec<DownloadConfig>,
    author: Option<String>,
    language: Option<String>,
}

pub fn read_download_config<P>(config: P) -> Result<HashMap<String, String>, Box<dyn std::error::Error>>
    where P: AsRef<Path>
{
    // Read the configuration file
    let config_content = fs::read_to_string(config)?;

    // Parse the TOML content
    let config: Config = toml::from_str(&config_content)?;

    // Create a HashMap to store the URLs and filenames
    let mut downloads = HashMap::new();

    // Populate the HashMap
    for download in config.download {
        downloads.insert(download.url, download.filename);
    }

    Ok(downloads)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {

        let conf = read_download_config("download.toml");

        println!("{:?}", conf);
        assert_eq!(conf.is_ok(), true);
    }
}
