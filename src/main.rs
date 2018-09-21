#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate log;
extern crate chrono;
extern crate csv;
extern crate env_logger;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
extern crate structopt;
#[macro_use]
extern crate serde_derive;
extern crate dirs;
extern crate git2;
extern crate regex;
extern crate toml;

use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

mod collector;
mod config;
mod database;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "tamatebako",
    about = "version checker for OSS Projects"
)]
struct CommandOption {
    #[structopt(long = "verbose", help = "verbose output")]
    verbose: bool,
    #[structopt(
        short = "c",
        long = "config",
        help = "config file",
        parse(from_os_str)
    )]
    config_file: PathBuf,
}

fn main() {
    env_logger::init();

    let opts = CommandOption::from_args();
    let config_filepath = opts
        .config_file
        .to_str()
        .expect("fail to get config filename");
    let config = config::load_config(config_filepath);
    let config = config.unwrap();
    debug!("config: {:?}", config);

    if !config.rootdir.exists() {
        if !fs::create_dir(&config.rootdir).is_ok() {
            return;
        }
    }

    if !env::set_current_dir(&config.rootdir).is_ok() {
        return;
    }

    let db_url = config.get_database_url();
    let dbconn = database::get_database_connection(db_url.as_str());
    database::create_table(&dbconn);

    for (project_name, project) in &config.projects {
        debug!("config.project: {:?}", project);

        match project.source {
            None => continue,
            Some(_) => {}
        }

        // NOTE: only git repo, now.
        let source = project.source.clone().unwrap();
        debug!("config.project: {:?}", source.git);

        let mut git_collector = collector::git::GitCollector::new(
            &db_url,
            &config.rootdir.to_str().unwrap(),
            &project_name,
            &source.git,
            &source.branch,
            &project.version_regex,
            config.git_ssh_key.clone(),
        );
        git_collector.init();

        // get version info
        git_collector.collect();
    }
}
