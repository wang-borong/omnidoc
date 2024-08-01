use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

//
// [[download]]
// url = ""
// filename = ""
//
#[derive(Deserialize, Debug)]
struct DownloadConfig {
    url: String,
    filename: String,
}

#[derive(Deserialize, Debug)]
struct Config {
    download: Vec<DownloadConfig>,
    author: Option<String>,
    language: Option<String>,
}

pub struct ConfigParser {
    config: Config,
}

impl ConfigParser {

    fn new<P>(config: P) -> Self
        where P: AsRef<Path>
    {
        let config_cont = fs::read_to_string(&config).unwrap_or("".to_string());
        let _config: Config = toml::from_str(&config_cont).unwrap();

        Self {
            config: _config,
        }
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>, Box<dyn std::error::Error>>
    {
        let config = &self.config;

        // Create a HashMap to store the URLs and filenames
        let mut downloads = HashMap::new();

        // Populate the HashMap
        for download in &config.download {
            downloads.insert(String::from(&download.url),
                String::from(&download.filename));
        }

        Ok(downloads)
    }

    pub fn get_author(&self) -> Result<String, Box<dyn std::error::Error>>
    {
        let config = &self.config;
        match &config.author {
            Some(author) => Ok(author.to_owned()),
            None => Err("no author configs".into())
        }
    }
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {

        let conf_parser = ConfigParser::new("omnidoc.toml");
        
        let downloads = conf_parser.get_downloads();

        println!("{:?}", downloads);
        assert_eq!(downloads.is_ok(), true);
    }
}
