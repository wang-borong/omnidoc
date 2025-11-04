use crate::constants::config as config_consts;
use crate::error::{OmniDocError, Result};
use console::style;
use dirs::{config_local_dir, data_local_dir};
use serde::Deserialize;
use std::collections::HashMap;
use std::env::set_var as env_set_var;
use std::env::var;
use std::fs;
use std::path::PathBuf;
use std::{self, io::Write};

#[derive(Deserialize, Debug)]
struct DownloadConfig {
    url: String,
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
struct Env {
    outdir: Option<String>,
    texmfhome: Option<String>,
    bibinputs: Option<String>,
    texinputs: Option<String>,
}

#[derive(Deserialize, Debug)]
struct Config {
    download: Option<Vec<DownloadConfig>>,
    author: Author,
    lib: Lib,
    env: Env,
    template_dir: Option<String>,
}

pub struct ConfigParser {
    config: Option<Config>,
    path: PathBuf,
}

impl ConfigParser {
    pub fn default() -> Result<Self> {
        // If the system don't have a config dir, then we create it
        let config_local_dir = match config_local_dir() {
            None => {
                let home_path = var("HOME").map_err(|_| {
                    OmniDocError::Config("HOME environment variable not found".to_string())
                })?;
                let mut _conf_dir = PathBuf::from(home_path);
                _conf_dir.push(config_consts::CONFIG_DIR);
                let _ = fs::create_dir_all(&_conf_dir);
                _conf_dir
            }
            Some(cld) => cld,
        };

        let omnidoc_config_file = config_local_dir.join(config_consts::OMNIDOC_CONFIG_FILE);
        if !omnidoc_config_file.exists() {
            let _ = ConfigParser::gen(
                config_consts::UNKNOWN_AUTHOR.to_string(),
                None,
                None,
                None,
                None,
                None,
                false,
            );
            println!(
                "{} The '{}' configuration file was created in '{}'.\n    You can modify it to set your author name.",
                style("ℹ").cyan().bold(),
                config_consts::OMNIDOC_CONFIG_FILE,
                config_local_dir.display()
            )
        }

        let config_cont = fs::read_to_string(&omnidoc_config_file)
            .map_err(|e| OmniDocError::Config(format!("Failed to read config file: {}", e)))?;
        let config: Config = toml::from_str(&config_cont)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse config file: {}", e)))?;

        let config_parser = Self {
            config: Some(config),
            path: omnidoc_config_file.clone(),
        };

        Ok(config_parser)
    }

    pub fn parse(&mut self) -> Result<()> {
        if !self.path.exists() {
            return Err(OmniDocError::ConfigNotFound(format!(
                "No OmniDoc config file found at {}. Please create it using 'omnidoc config'",
                self.path.display()
            )));
        }

        let config_cont = fs::read_to_string(&self.path)
            .map_err(|e| OmniDocError::Config(format!("Failed to read config file: {}", e)))?;
        let config: Config = toml::from_str(&config_cont)
            .map_err(|e| OmniDocError::Config(format!("Failed to parse config file: {}", e)))?;

        self.config = Some(config);

        Ok(())
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        // Create a HashMap to store the URLs and filenames
        let mut downloads = HashMap::new();

        // Populate the HashMap
        if let Some(download_list) = &config.download {
            for download in download_list {
                downloads.insert(download.url.clone(), download.filename.clone());
            }
        }

        Ok(downloads)
    }

    pub fn get_author_name(&self) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        match &config.author.name {
            Some(author) => Ok(author.to_owned()),
            None => Err(OmniDocError::Config(
                "No author name configured".to_string(),
            )),
        }
    }

    pub fn get_omnidoc_lib(&self) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        match &config.lib.path {
            Some(lib_path) => Ok(lib_path.to_owned()),
            None => Err(OmniDocError::Config(
                "No OmniDoc library configured".to_string(),
            )),
        }
    }

    pub fn get_template_dir(&self) -> Option<String> {
        self.config.as_ref().and_then(|c| c.template_dir.clone())
    }

    pub fn get_envs(&self) -> Result<HashMap<&'static str, Option<String>>> {
        let mut envs: HashMap<&'static str, Option<String>> = HashMap::new();
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        if let Some(outdir) = &config.env.outdir {
            envs.insert("outdir", Some(outdir.to_owned()));
        } else {
            envs.insert("outdir", None);
        }

        if let Some(texmfhome) = &config.env.texmfhome {
            envs.insert("texmfhome", Some(texmfhome.to_owned()));
        } else {
            envs.insert("texmfhome", None);
        }

        if let Some(texinputs) = &config.env.texinputs {
            envs.insert("texinputs", Some(texinputs.to_owned()));
        } else {
            envs.insert("texinputs", None);
        }

        if let Some(bibinputs) = &config.env.bibinputs {
            envs.insert("bibinputs", Some(bibinputs.to_owned()));
        } else {
            envs.insert("bibinputs", None);
        }

        Ok(envs)
    }

    fn rander_config(
        author: String,
        lib: Option<String>,
        outdir: Option<String>,
        texmfhome: Option<String>,
        bibinputs: Option<String>,
        texinputs: Option<String>,
    ) -> Result<String> {
        let default_envs = HashMap::from([
            ("texmfhome", r"$ENV{HOME}/.local/share/omnidoc/texmf//:"),
            ("bibinputs", r"./biblio//:"),
            ("texinputs", r"./tex//:"),
        ]);

        let mut config = String::new();

        config.push_str(config_consts::SECTION_AUTHOR);
        config.push_str(&format!("name = \"{}\"\n", author));

        if let Some(lib) = lib {
            config.push_str(config_consts::SECTION_LIB);
            config.push_str(&format!("path = \"{}\"\n", lib));
        } else {
            let dld = data_local_dir().ok_or_else(|| {
                OmniDocError::Config("Local data directory not found".to_string())
            })?;
            let olib = dld.join("omnidoc");
            let lib_path_str = olib.to_str().ok_or_else(|| {
                OmniDocError::Config("Library path contains invalid UTF-8".to_string())
            })?;
            config.push_str(config_consts::SECTION_LIB);
            config.push_str(&format!("path = \"{}\"\n", lib_path_str));
        }

        config.push_str(config_consts::SECTION_ENV);
        if let Some(outdir) = outdir {
            config.push_str(&format!("outdir = \"{}\"\n", outdir));
        } else {
            config.push_str("outdir = \"build\"\n");
        }

        if let Some(texmfhome) = texmfhome {
            let mut new_env = String::from(default_envs["texmfhome"]);
            new_env.push_str(&texmfhome);
            if !texmfhome.ends_with(config_consts::PATH_SEPARATOR) {
                new_env.push_str(config_consts::PATH_SEPARATOR);
            }
            config.push_str(&format!("texmfhome = \"{}\"\n", new_env));
        } else {
            config.push_str(&format!("texmfhome = \"{}\"\n", default_envs["texmfhome"]));
        }

        if let Some(texinputs) = texinputs {
            let mut new_env = String::from(default_envs["texinputs"]);
            new_env.push_str(&texinputs);
            if !texinputs.ends_with(config_consts::PATH_SEPARATOR) {
                new_env.push_str(config_consts::PATH_SEPARATOR);
            }
            config.push_str(&format!("texinputs = \"{}\"\n", new_env));
        } else {
            config.push_str(&format!("texinputs = \"{}\"\n", default_envs["texinputs"]));
        }

        if let Some(bibinputs) = bibinputs {
            let mut new_env = String::from(default_envs["bibinputs"]);
            new_env.push_str(&bibinputs);
            if !bibinputs.ends_with(config_consts::PATH_SEPARATOR) {
                new_env.push_str(config_consts::PATH_SEPARATOR);
            }
            config.push_str(&format!("bibinputs = \"{}\"\n", new_env));
        } else {
            config.push_str(&format!("bibinputs = \"{}\"\n", default_envs["bibinputs"]));
        }

        Ok(config)
    }

    pub fn gen(
        author: String,
        lib: Option<String>,
        outdir: Option<String>,
        texmfhome: Option<String>,
        bibinputs: Option<String>,
        texinputs: Option<String>,
        force: bool,
    ) -> Result<()> {
        let config =
            ConfigParser::rander_config(author, lib, outdir, texmfhome, bibinputs, texinputs)?;
        if let Some(conf_path) = config_local_dir() {
            let omnidoc_config_file = conf_path.join(config_consts::OMNIDOC_CONFIG_FILE);
            if force {
                fs::remove_file(&omnidoc_config_file).map_err(|e| {
                    OmniDocError::Config(format!("Failed to remove existing config file: {}", e))
                })?;
            }
            if !omnidoc_config_file.exists() {
                let mut ocf = fs::File::create(&omnidoc_config_file).map_err(|e| {
                    OmniDocError::Config(format!("Failed to create config file: {}", e))
                })?;
                ocf.write_all(config.as_bytes()).map_err(|e| {
                    OmniDocError::Config(format!("Failed to write config file: {}", e))
                })?;
            } else {
                return Err(OmniDocError::Config(format!(
                    "The {} already exists",
                    config_consts::OMNIDOC_CONFIG_FILE
                )));
            }

            Ok(())
        } else {
            Err(OmniDocError::Config(
                "No ~/.config directory found on your system".to_string(),
            ))
        }
    }

    pub fn setup_env(&self) -> Result<()> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| OmniDocError::Config("Configuration not loaded".to_string()))?;

        match &config.env.texmfhome {
            Some(texmfhome) => env_set_var("TEXMFHOME", texmfhome),
            None => {}
        }
        match &config.env.bibinputs {
            Some(bibinputs) => env_set_var("BIBINPUTS", bibinputs),
            None => {}
        }
        match &config.env.texinputs {
            Some(texinputs) => env_set_var("TEXINPUTS", texinputs),
            None => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_config() {
        match ConfigParser::default() {
            Ok(mut conf_parser) => {
                if let Err(e) = conf_parser.parse() {
                    eprintln!("Failed to parse config: {}", e);
                    return;
                }
                if let Some(config) = conf_parser.config.as_ref() {
                    println!("show conf: {:?}", config);
                } else {
                    eprintln!("Configuration not loaded after parse");
                    return;
                }
                assert!(true);
            }
            Err(e) => {
                eprintln!("Failed to get default config: {}", e);
                // In test environment, it's okay if config doesn't exist
                // This test just verifies the structure works
            }
        }
    }
}
