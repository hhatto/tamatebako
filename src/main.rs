#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate log;
extern crate chrono;
extern crate csv;
extern crate env_logger;
extern crate reqwest;
extern crate url;
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
extern crate serde_json;
extern crate toml;
extern crate actix;
extern crate actix_web;

use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

mod collector;
mod config;
mod database;
mod web;

#[derive(Debug, StructOpt)]
#[structopt(name = "tamatebako", about = "version checker for OSS Projects")]
struct CommandOption {
    #[structopt(long = "log-level", help = "logging level", default_value = "info")]
    log_level: String,
    #[structopt(short = "c", long = "config", help = "config file", parse(from_os_str))]
    config_file: PathBuf,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// check and store version history information
    #[structopt(name = "check")]
    CheckCommand,

    /// serve version history visualize web application
    #[structopt(name = "web")]
    WebCommand,
}

#[derive(Debug, StructOpt)]
struct CheckCommand {}

#[derive(Debug, StructOpt)]
struct WebCommand {}

fn main() {
    let opts = CommandOption::from_args();
    println!("{:?}", opts);
    match opts.log_level.as_str() {
        "debug" | "info" | "warn" | "error" => {},
        _ => {
            println!("invalid log-level");
            return;
        },
    }
    let env = env_logger::Env::default()
        .filter_or(env_logger::DEFAULT_FILTER_ENV, opts.log_level);
    env_logger::Builder::from_env(env).init();

    let config_filepath = opts.config_file.to_str().expect("fail to get config filename");
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

    match opts.cmd {
        Command::WebCommand => {
            web::serve();
        },
        Command::CheckCommand => {
            for (project_name, project) in &config.projects {
                debug!("config.project: {:?}", project);

                match project.source {
                    None => continue,
                    Some(_) => {}
                }

                let source = project.source.clone().unwrap();

                debug!("config.project: {:?}", source.git);
                match source.git {
                    Some(git) => {
                        let branch = match source.branch {
                            Some(b) => b,
                            None => "master".to_string(),
                        };
                        let mut git_collector = collector::git::GitCollector::new(
                            &db_url,
                            &config.rootdir.to_str().unwrap(),
                            &git,
                            &project_name,
                            &project.url,
                            &branch,
                            &project.version_regex,
                            config.git_ssh_key.clone(),
                        );
                        git_collector.init();

                        // get version info
                        git_collector.collect();
                    }
                    None => {}
                }

                debug!("config.project: {:?}", source.github);
                match source.github {
                    Some(github_repo) => {
                        let tmp: Vec<&str> = github_repo.split("/").collect();
                        let owner = tmp[0];
                        let repo = tmp[1];
                        let github_collector = collector::github::GitHubCollector::new(
                            &db_url,
                            &project_name,
                            owner,
                            repo,
                            config.github_access_token.clone(),
                        );
                        let _ = github_collector.get_releases();
                    }
                    None => {}
                }
            }
        },
    }
}
