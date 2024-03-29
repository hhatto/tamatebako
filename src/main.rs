#![allow(proc_macro_derive_resolution_fallback)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::{env, fs};

mod collector;
mod config;
mod database;
mod web;

#[derive(Parser)]
#[command(
    name = env!("CARGO_PKG_NAME"),
    version = env!("CARGO_PKG_VERSION"),
    about = env!("CARGO_PKG_DESCRIPTION"),
    arg_required_else_help = true,
)]
struct Cli {
    #[arg(long, help = "logging level", default_value = "info")]
    log_level: String,
    #[arg(short, long = "config", help = "config file")]
    config_file: Option<PathBuf>,
    #[command(subcommand)]
    cmd: SubCommand,
}

#[derive(Subcommand)]
enum SubCommand {
    /// check and store version history information
    Check {},

    /// output the latest version of each projects
    List {
        #[arg(short = 's', long = "sort", help = "sort key", value_enum)]
        sort_key: Option<ListSortKey>,
        #[arg(short = 'r', long = "reverse", help = "reverse the order of the sort item")]
        reverse: bool,
    },

    /// serve version history visualize web application
    Web {},
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ListSortKey {
    Name,
    Version,
    DateTime,
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    let cli = Cli::parse();
    match cli.log_level.as_str() {
        "debug" | "info" | "warn" | "error" => {}
        _ => {
            println!("invalid log-level");
            return Ok(());
        }
    }
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, cli.log_level);
    env_logger::Builder::from_env(env).init();

    let default_config_path = config::default_config_path();
    let config_filepath = if default_config_path.exists() {
        default_config_path
            .as_path()
            .to_str()
            .expect("fail to get default config filename")
            .to_string()
    } else {
        match cli.config_file {
            Some(c) => c.to_str().expect("fail to get config filename").to_string(),
            None => {
                error!("not exists default config file: {:?}", default_config_path);
                error!("config file is not exists");
                error!("execute with -c/--config option or set config file to default path");
                return Ok(());
            }
        }
    };
    let config = config::load_config(config_filepath.as_str()).expect("fail to load config");
    debug!("config: {:?}", config);

    if !config.rootdir.exists() && fs::create_dir(&config.rootdir).is_err() {
        return Ok(());
    }

    if env::set_current_dir(&config.rootdir).is_err() {
        return Ok(());
    }

    let db_url = config.get_database_url();
    let mut dbconn = database::get_database_connection(db_url.as_str());
    database::create_table(&mut dbconn);

    match cli.cmd {
        SubCommand::Web {} => {
            web::serve();
        }
        SubCommand::List { sort_key, reverse } => {
            let order_by = match sort_key {
                Some(ListSortKey::Name) => "project_name",
                Some(ListSortKey::Version) => "version",
                Some(ListSortKey::DateTime) => "bump_date",
                _ => "project_name",
            };
            let version_histories =
                database::get_latest_version_history(&mut dbconn, Some(order_by.to_string()), reverse);
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
        SubCommand::Check {} => {
            for (project_name, project) in &config.projects {
                debug!("config.project: {:?}", project);

                match project.source {
                    None => continue,
                    Some(_) => {}
                }

                let mut new_release_versions = 0;
                let source = project.source.clone().unwrap();

                debug!("config.source.git: {:?}", source.git);
                if let Some(git) = source.git {
                    let branch = match source.branch {
                        Some(b) => b,
                        None => "master".to_string(),
                    };
                    let mut git_collector = collector::git::GitCollector::new(
                        &db_url,
                        config.rootdir.to_str().unwrap(),
                        &git,
                        project_name,
                        &project.url,
                        &branch,
                        &project.version_regex,
                        config.git_ssh_key.clone(),
                    );
                    git_collector.init();

                    // get version info
                    new_release_versions = git_collector.collect();
                }

                debug!("config.source.github: {:?}", source.github);
                if let Some(github_repo) = source.github {
                    let tmp: Vec<&str> = github_repo.split('/').collect();
                    let owner = tmp[0];
                    let repo = tmp[1];
                    let github_collector = collector::github::GitHubCollector::new(
                        &db_url,
                        project_name,
                        owner,
                        repo,
                        config.github_access_token.clone(),
                    );
                    match github_collector.get_releases().await {
                        Ok(n) => new_release_versions = n,
                        Err(e) => error!("github collector error: {:#?}", e),
                    }
                }

                if new_release_versions == 0 {
                    info!("not exist new version(s): {}", project_name);
                }
            }
        }
    }
    Ok(())
}
