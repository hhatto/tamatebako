use std::io;
use std::fs::File;
use std::io::Read;
use std::collections::HashMap;
use toml;

#[derive(Debug)]
pub struct Config {
    pub projects: HashMap<String, ProjectConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectConfig {
    pub url: String,
}

impl Config {
    pub fn new() -> Config {
        Self {projects: HashMap::new()}
    }
}

pub fn load_config(path: &str) -> io::Result<Config> {
    let mut config_toml = String::new();
    let mut config = Config::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut config_toml)?;

    let mut projects: HashMap<String, ProjectConfig> = toml::from_str(config_toml.as_str()).expect("parse toml error");

    config.projects = projects;
    Ok(config)
}
