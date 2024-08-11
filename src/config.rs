use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::error::Error;
use dirs::config_local_dir;
use std::io::Write;

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
    download: Option<Vec<DownloadConfig>>,
    author:   Author,
    lib:      Lib,
}

pub struct ConfigParser {
    config: Option<Config>,
    path:   PathBuf,
}

impl ConfigParser {

    pub fn from<P>(config: P) -> Self
        where P: AsRef<Path>
    {
        let conf = PathBuf::new();

        Self {
            config: None,
            path: conf.join(&config),
        }
    }

    pub fn default() -> Self
    {
        let config_local_dir = config_local_dir().expect("No ~/.config in your system");
        let omnidoc_config_file = config_local_dir.join("omnidoc.toml");

        Self {
            config: None,
            path: omnidoc_config_file,
        }
    }

    pub fn parse(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.path.exists() {
            return Err("No omnidoc config file, please create it by 'omnidoc config'".into());
        }

        let config_cont = fs::read_to_string(&self.path).unwrap_or("".to_string());
        let config: Config = toml::from_str(&config_cont).expect("can not parse configs");

        self.config = Some(config);

        Ok(())
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>, Box<dyn Error>>
    {
        let config = self.config.as_ref().unwrap();

        // Create a HashMap to store the URLs and filenames
        let mut downloads = HashMap::new();

        // Populate the HashMap
        for download in config.download.as_ref().unwrap() {
            downloads.insert(String::from(&download.url),
                String::from(&download.filename));
        }

        Ok(downloads)
    }

    pub fn get_author_name(&self) -> Result<String, Box<dyn Error>>
    {
        let config = self.config.as_ref().unwrap();

        match &config.author.name {
            Some(author) => Ok(author.to_owned()),
            None => Err("No author name configured".into())
        }
    }

    pub fn get_omnidoc_lib(&self) -> Result<String, Box<dyn Error>>
    {
        let config = self.config.as_ref().unwrap();

        match &config.lib.path {
            Some(lib_path) => Ok(lib_path.to_owned()),
            None => Err("No omnidoc lib configured".into()),
        }
    }

    pub fn gen(&self) -> Result<(), Box<dyn Error>>
    {
        let config = include_str!("../assets/omnidoc.toml");
        if let Some(conf_path) = config_local_dir() {
            let omnidoc_config_file = conf_path.join("omnidoc.toml");
            if !omnidoc_config_file.exists() {
                let mut ocf = fs::File::create(&omnidoc_config_file)?;
                ocf.write_all(config.as_bytes())?;
            } else {
                return Err("The omnidoc.toml already exists".into());
            }

            Ok(())
        } else {
            return Err("No ~/.config in your system".into());
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
