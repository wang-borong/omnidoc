use dirs::{config_local_dir, data_local_dir};
use serde::Deserialize;
use std::collections::HashMap;
use std::env::set_var as env_set_var;
use std::env::var;
use std::error::Error;
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
}

pub struct ConfigParser {
    config: Option<Config>,
    path: PathBuf,
}

impl ConfigParser {
    pub fn default() -> Result<Self, std::io::Error> {
        // If the system don't have a config dir, then we create it
        let config_local_dir = match config_local_dir() {
            None => {
                let home_path = var("HOME").map_err(|_| std::io::Error::new(std::io::ErrorKind::NotFound, "HOME env not found"))?;
                let mut _conf_dir = PathBuf::from(home_path);
                _conf_dir.push(".config");
                let _ = fs::create_dir_all(&_conf_dir);
                _conf_dir
            },
            Some(cld) => cld,
        };

        let omnidoc_config_file = config_local_dir.join("omnidoc.toml");
        if !omnidoc_config_file.exists() {
            let _ = ConfigParser::gen("unknown".to_string(), None, None, None, None, None, false);
            println!("The 'omnidoc.toml' configuration file was created in '{}'.\n\
                You can modify it to change the author to yours.",
                config_local_dir.display())
        }

        let config_cont = fs::read_to_string(&omnidoc_config_file)?;
        let config: Config = toml::from_str(&config_cont)?;

        let config_parser = Self {
            config: Some(config),
            path: omnidoc_config_file.clone(),
        };

        Ok(config_parser)
    }

    pub fn parse(&mut self) -> Result<(), Box<dyn Error>> {
        if !self.path.exists() {
            return Err("No omnidoc config file, please create it by 'omnidoc config'".into());
        }

        let config_cont = fs::read_to_string(&self.path).unwrap_or("".to_string());
        let config: Config = toml::from_str(&config_cont)?;

        self.config = Some(config);

        Ok(())
    }

    pub fn get_downloads(&self) -> Result<HashMap<String, String>, Box<dyn Error>> {
        let config = self.config.as_ref().ok_or("Config not loaded")?;

        // Create a HashMap to store the URLs and filenames
        let mut downloads = HashMap::new();

        // Populate the HashMap
        if let Some(download_list) = &config.download {
            for download in download_list {
                downloads.insert(
                    download.url.clone(),
                    download.filename.clone(),
                );
            }
        }

        Ok(downloads)
    }

    pub fn get_author_name(&self) -> Result<String, Box<dyn Error>> {
        let config = self.config.as_ref().ok_or("Config not loaded")?;

        match &config.author.name {
            Some(author) => Ok(author.to_owned()),
            None => Err("No author name configured".into()),
        }
    }

    pub fn get_omnidoc_lib(&self) -> Result<String, Box<dyn Error>> {
        let config = self.config.as_ref().ok_or("Config not loaded")?;

        match &config.lib.path {
            Some(lib_path) => Ok(lib_path.to_owned()),
            None => Err("No omnidoc lib configured".into()),
        }
    }

    pub fn get_envs(&self) -> Result<HashMap<&str, Option<String>>, Box<dyn Error>> {
        let mut envs: HashMap<&str, Option<String>> = HashMap::new();
        let config = self.config.as_ref().ok_or("Config not loaded")?;

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
    ) -> Result<String, Box<dyn Error>> {
        let default_envs = HashMap::from([
            ("texmfhome", r"$ENV{HOME}/.local/share/omnidoc/texmf//:"),
            ("bibinputs", r"./biblio//:"),
            ("texinputs", r"./tex//:"),
        ]);

        let mut config = String::new();

        config.push_str("[author]\n");
        config.push_str(&format!("name = \"{}\"\n", author));

        if let Some(lib) = lib {
            config.push_str(&format!("[lib]\npath = \"{}\"\n", lib));
        } else {
            let dld = data_local_dir().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "data_local_dir not found"))?;
            let olib = dld.join("omnidoc");
            config.push_str(&format!("[lib]\npath = \"{}\"\n", olib.to_str().unwrap_or("lib path not found")))
        }

        config.push_str("[env]\n");
        if let Some(outdir) = outdir {
            config.push_str(&format!("outdir = \"{}\"\n", outdir));
        } else {
            config.push_str("outdir = \"build\"\n")
        }

        if let Some(texmfhome) = texmfhome {
            let mut new_env = String::from(default_envs["texmfhome"]);
            new_env.push_str(&texmfhome);
            if !texmfhome.ends_with("/:") {
                new_env.push_str("/:")
            }
            config.push_str(&format!("texmfhome = \"{}\"\n", new_env))
        } else {
            config.push_str(&format!("texmfhome = \"{}\"\n", default_envs["texmfhome"]))
        }

        if let Some(texinputs) = texinputs {
            let mut new_env = String::from(default_envs["texinputs"]);
            new_env.push_str(&texinputs);
            if !texinputs.ends_with("/:") {
                new_env.push_str("/:")
            }
            config.push_str(&format!("texinputs = \"{}\"\n", new_env))
        } else {
            config.push_str(&format!("texinputs = \"{}\"\n", default_envs["texinputs"]))
        }

        if let Some(bibinputs) = bibinputs {
            let mut new_env = String::from(default_envs["bibinputs"]);
            new_env.push_str(&bibinputs);
            if !bibinputs.ends_with("/:") {
                new_env.push_str("/:")
            }
            config.push_str(&format!("bibinputs = \"{}\"\n", new_env))
        } else {
            config.push_str(&format!("bibinputs = \"{}\"\n", default_envs["bibinputs"]))
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
    ) -> Result<(), Box<dyn Error>> {
        let config =
            ConfigParser::rander_config(author, lib, outdir, texmfhome, bibinputs, texinputs)?;
        if let Some(conf_path) = config_local_dir() {
            let omnidoc_config_file = conf_path.join("omnidoc.toml");
            if force {
                fs::remove_file(&omnidoc_config_file)?;
            }
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

    pub fn setup_env(&self) -> Result<(), Box<dyn Error>> {
        let config = self.config.as_ref().ok_or("Config not loaded")?;

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
        let mut conf_parser = ConfigParser::default().expect("Get default config failed");
        let _ = conf_parser.parse();

        let config = conf_parser.config.as_ref().expect("Config not loaded");
        println!("show conf: {:?}", config);

        assert!(true);
    }
}
