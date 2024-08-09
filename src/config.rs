use serde::Deserialize;
use core::panic;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::error::Error;
use dirs::config_local_dir;

//
// [[download]]
// url = ""
// filename = ""
//
#[derive(Deserialize, Debug)]
struct DownloadConfig {
    url:      String,
    filename: String,
}

#[derive(Deserialize, Debug)]
struct Author {
    name: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Lib {
    path: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    download: Vec<DownloadConfig>,
    author:   Author,
    lib:      Lib,
}

pub struct ConfigParser {
    config: Config,
}

impl ConfigParser {

    pub fn from<P>(config: P) -> Self
        where P: AsRef<Path>
    {
        let config_cont = fs::read_to_string(&config).unwrap_or("".to_string());
        let config: Config = toml::from_str(&config_cont).expect("can not parse configs");

        Self {
            config, // shorthand
        }
    }

    pub fn default() -> Self
    {
        let config_local_dir = config_local_dir().expect("no config dir in your local");
        let omnidoc_config_file = config_local_dir.join("omnidoc.toml");

        if !omnidoc_config_file.exists() {
            panic!("no omnidoc config file");
        }

        let config_cont = fs::read_to_string(&omnidoc_config_file).unwrap_or("".to_string());
        let config: Config = toml::from_str(&config_cont).expect("can not parse configs");

        Self {
            config, // shorthand
        }
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>, Box<dyn Error>>
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

    pub fn get_author_name(&self) -> Result<String, Box<dyn Error>>
    {
        let config = &self.config;

        match &config.author.name {
            Some(author) => Ok(author.to_owned()),
            None => Err("no author name configured".into())
        }
    }

    pub fn get_omnidoc_lib(&self) -> Result<String, Box<dyn Error>>
    {
        let config = &self.config;

        match &config.lib.path {
            Some(lib_path) => Ok(lib_path.to_owned()),
            None => Err("no omnidoc lib configured".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {

        let conf_parser = ConfigParser::from("omnidoc.toml");
        
        let downloads = conf_parser.get_downloads();

        println!("{:?}", downloads);
        assert_eq!(downloads.is_ok(), true);
    }
}
