use dirs;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use toml;

#[derive(Clone, Debug)]
pub struct Config {
    pub rootdir: PathBuf,
    pub projects: HashMap<String, ProjectConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectConfig {
    pub url: String,
    pub source: Option<ProjectSourceConfig>,
    pub version_regex: Option<String>,
}

impl Config {
    pub fn new() -> Config {
        Self {
            rootdir: PathBuf::from(""),
            projects: HashMap::new(),
        }
    }

    pub fn get_database_url(&self) -> String {
        format!("{}/tamatebako.sqlite", self.rootdir.to_str().unwrap())
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ProjectSourceConfig {
    pub git: String,
    pub branch: String,
}

pub fn load_config(path: &str) -> io::Result<Config> {
    let mut config_toml = String::new();
    let mut config = Config::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut config_toml)?;

    let projects: HashMap<String, ProjectConfig> =
        toml::from_str(config_toml.as_str()).expect("parse toml error");

    config.projects = projects;
    config.rootdir = PathBuf::from(format!(
        "{}/.tamatebako",
        dirs::home_dir()
            .expect("fail get homedir")
            .to_str()
            .unwrap()
    ));
    Ok(config)
}
