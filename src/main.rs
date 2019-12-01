#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate serde_derive;

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
    config_file: Option<PathBuf>,
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// check and store version history information
    #[structopt(name = "check")]
    Check {},

    /// output the latest version of each projects
    #[structopt(name = "list")]
    List {
        #[structopt(short = "s", long = "sort", help = "sort key",
                    possible_values = &ListSortKey::variants(), case_insensitive = true)]
        sort_key: Option<ListSortKey>,
        #[structopt(short = "r", long = "reverse", help = "reverse the order of the sort item")]
        reverse: bool,
    },

    /// serve version history visualize web application
    #[structopt(name = "web")]
    Web {},
}

arg_enum! {
    #[derive(Debug)]
    enum ListSortKey {
        Name,
        Version,
        DateTime,
    }
}

fn main() {
    let opts = CommandOption::from_args();
    match opts.log_level.as_str() {
        "debug" | "info" | "warn" | "error" => {}
        _ => {
            println!("invalid log-level");
            return;
        }
    }
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, opts.log_level);
    env_logger::Builder::from_env(env).init();

    let default_config_path = config::default_config_path();
    let config_filepath = if default_config_path.exists() {
        default_config_path
            .as_path()
            .to_str()
            .expect("fail to get default config filename")
            .to_string()
    } else {
        match opts.config_file {
            Some(c) => c.to_str().expect("fail to get config filename").to_string(),
            None => {
                panic!("config file is not exists");
            }
        }
    };
    let config = config::load_config(config_filepath.as_str()).expect("fail to load config");
    debug!("config: {:?}", config);

    if !config.rootdir.exists() && fs::create_dir(&config.rootdir).is_err() {
        return;
    }

    if env::set_current_dir(&config.rootdir).is_err() {
        return;
    }

    let db_url = config.get_database_url();
    let dbconn = database::get_database_connection(db_url.as_str());
    database::create_table(&dbconn);

    match Command::from_args() {
        Command::Web {} => {
            web::serve();
        }
        Command::List { sort_key, reverse } => {
            let order_by = match sort_key {
                Some(ListSortKey::Name) => "project_name",
                Some(ListSortKey::Version) => "version",
                Some(ListSortKey::DateTime) => "bump_date",
                _ => "project_name",
            };
            let version_histories = database::get_latest_version_history(&dbconn, Some(order_by.to_string()), reverse);
            let mut name_max_len = 0;
            for version_history in &version_histories {
                if name_max_len < version_history.project_name.len() {
                    name_max_len = version_history.project_name.len();
                }
            }
            for version_history in &version_histories {
                println!(
                    "{name:>width$}: {version:<10} ({date})",
                    name = version_history.project_name,
                    width = name_max_len,
                    version = version_history.version,
                    date = version_history.bump_date
                );
            }
        }
        Command::Check {} => {
            for (project_name, project) in &config.projects {
                debug!("config.project: {:?}", project);

                match project.source {
                    None => continue,
                    Some(_) => {}
                }

                let source = project.source.clone().unwrap();

                debug!("config.project: {:?}", source.git);
                if let Some(git) = source.git {
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

                debug!("config.project: {:?}", source.github);
                if let Some(github_repo) = source.github {
                    let tmp: Vec<&str> = github_repo.split('/').collect();
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
            }
        }
    }
}
