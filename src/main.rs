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

use chrono::NaiveDateTime;
use git2::Repository;
use regex::Regex;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, fs};
use structopt::StructOpt;

mod config;
mod database;

lazy_static! {
    static ref RE_GIT_DIR: Regex = Regex::new(r"^(https://|git@)(.*).git$").unwrap();
    static ref RE_GIT_TAG: Regex = Regex::new(r"tag: (.*)(, .*)?").unwrap();
}

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

#[derive(Debug, Deserialize)]
struct GitInfo {
    tag: String,
    date: String,
    hash: String,
}

fn main() {
    env_logger::init();

    let opts = CommandOption::from_args();
    let config_filepath = opts.config_file;
    let config = config::load_config(
        config_filepath
            .to_str()
            .expect("fail to get config filename"),
    );
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
        let source = project.source.clone().unwrap();
        debug!("config.project: {:?}", source.git);

        let git_directory = match RE_GIT_DIR.captures(&source.git) {
            Some(caps) => format!(
                "{}/{}",
                &config.rootdir.to_str().unwrap(),
                caps.get(2).unwrap().as_str()
            ),
            None => "".to_string(),
        };
        if git_directory.is_empty() {
            error!("not found git source");
            continue;
        }

        let _repo = match Repository::open(&git_directory) {
            Ok(repo) => repo,
            Err(_) => {
                // clone
                match Repository::clone(&source.git, &git_directory) {
                    Ok(repo) => repo,
                    Err(e) => panic!("fail git clone. error: {:?}", e),
                }
            }
        };

        if !env::set_current_dir(&git_directory).is_ok() {
            return;
        }

        // set branch
        let git_branch = &source.branch;
        info!("repo: {}, branch: {}", source.git, git_branch);
        let _proc = Command::new("git")
            .arg("checkout")
            .arg(format!("{}", git_branch))
            .output()
            .expect("fail git checkout command");

        // get version info
        let mut git_proc = Command::new("git")
            .arg("log")
            .arg("-n 100")
            .arg("--oneline")
            .arg("--date=format:%Y/%m/%d %H:%M:%S")
            .arg("--pretty=format:\"%D\",\"%cd\",\"%H\"")
            .stdout(Stdio::piped())
            .spawn()
            .expect("fail git log command");
        let mut grep_proc = Command::new("grep")
            .arg("tag: ")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("fail grep command");

        match git_proc.wait() {
            Ok(out) => {
                if !out.success() {
                    panic!("fail git command");
                }
            },
            Err(e) => panic!("fail git command. {:?}", e),
        }

        if let Some(ref mut stdout) = git_proc.stdout {
            if let Some(ref mut stdin) = grep_proc.stdin {
                let mut buf: Vec<u8> = Vec::new();
                stdout.read_to_end(&mut buf).unwrap();
                stdin.write_all(&buf).unwrap();
            }
        }
        match grep_proc.wait() {
            Ok(out) => {
                if !out.success() {
                    panic!("fail grep command");
                }
            },
            Err(e) => panic!("fail grep command. {:?}", e),
        }

        let re_version = match &project.version_regex {
            Some(s) => Some(Regex::new(&s).unwrap()),
            None => None,
        };

        let reader = BufReader::new(grep_proc.stdout.unwrap());
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);
        for row in rdr.deserialize() {
            let mut record: GitInfo = row.expect("fail deserialize csv data");
            record.tag = {
                match RE_GIT_TAG.captures(&record.tag) {
                    Some(caps) => {
                        let c = caps.get(1).unwrap().as_str().to_string();
                        match &re_version {
                            Some(vregex) => {
                                match vregex.captures(&c) {
                                    Some(caps) => caps.get(1).unwrap().as_str().to_string(),
                                    None => "".to_string(),
                                }
                            },
                            None => c,
                        }
                    },
                    None => "".to_string(),
                }
            };
            debug!("record: {:?}", record);
            if record.tag.is_empty() {
                continue;
            }

            let bump_date =
                NaiveDateTime::parse_from_str(record.date.as_str(), "%Y/%m/%d %H:%M:%S")
                    .expect("fail parse date");
            let version_history = database::VersionHistory {
                id: 0,
                project_name: project_name.clone(),
                channel: source.branch.clone(),
                version: record.tag.clone(),
                bump_date: bump_date,
                url: Some(format!("{}/releases/tag/{}", project.url, record.tag)),
            };

            match database::insert_version_history(&dbconn, version_history) {
                Ok(n) => debug!("insert {} data", n),
                Err(e) => error!("insert error: {:?}", e),
            }
        }

        if !env::set_current_dir(&config.rootdir).is_ok() {
            return;
        }
    }
}
