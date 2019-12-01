use chrono::NaiveDateTime;
use csv;
use git2;
use git2::build::RepoBuilder;
use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};
use regex::Regex;
use std::env;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::database;

lazy_static! {
    static ref RE_GIT_DIR: Regex = Regex::new(r"^(https://|git@)(.*).git$").unwrap();
    static ref RE_GIT_TAG: Regex = Regex::new(r"tag: (.*)(, .*)?").unwrap();
}

#[derive(Debug, Deserialize)]
struct GitInfo {
    tag: String,
    date: String,
    hash: String,
}

#[derive(Debug, Default)]
pub struct GitCollector {
    db_url: String,
    clone_url: String,
    url: String,
    project_name: String,
    branch: String,
    directory: String,
    version_regex: Option<Regex>,
    ssh_key: Option<String>,
}

fn git_clone(url: &str, directory: &str, ssh_key: &Option<String>) -> Result<Repository, git2::Error> {
    match ssh_key {
        Some(key) => {
            let mut builder = RepoBuilder::new();
            let mut callbacks = RemoteCallbacks::new();
            let mut fetch_options = FetchOptions::new();

            callbacks.credentials(|_, _, _| {
                let pubkey_path = format!("{}.pub", key);
                let privatekey_path = key.to_string();
                let credentials = Cred::ssh_key(
                    "git",
                    Some(Path::new(&pubkey_path)),
                    Path::new(privatekey_path.as_str()),
                    None,
                )
                .expect("fail crate credentials");
                Ok(credentials)
            });
            fetch_options.remote_callbacks(callbacks);
            builder.fetch_options(fetch_options);

            info!("git clone {}", url);
            builder.clone(url, Path::new(directory))
        }
        None => Repository::clone(url, directory),
    }
}

impl GitCollector {
    pub fn new(
        db_url: &str,
        rootdir: &str,
        clone_url: &str,
        project_name: &str,
        url: &str,
        branch: &str,
        version_regex: &Option<String>,
        ssh_key: Option<String>,
    ) -> Self {
        let git_directory = match RE_GIT_DIR.captures(clone_url) {
            Some(caps) => {
                let directory = caps.get(2).unwrap();
                format!("{}/{}", rootdir, directory.as_str().replace(":", "/"))
            }
            None => "".to_string(),
        };
        let re_version = match version_regex {
            Some(s) => Some(Regex::new(s.as_str()).unwrap()),
            None => None,
        };

        Self {
            db_url: db_url.to_string(),
            clone_url: clone_url.to_string(),
            url: url.to_string(),
            project_name: project_name.to_string(),
            branch: branch.to_string(),
            directory: git_directory,
            version_regex: re_version,
            ssh_key,
        }
    }

    pub fn init(&mut self) {
        let old_curdir = env::current_dir().unwrap();

        if self.directory.is_empty() {
            // TODO: error handling
            error!("not found git repo directory");
            return;
        }

        let _repo = match Repository::open(&self.directory) {
            Ok(repo) => repo,
            Err(_) => match git_clone(&self.clone_url, &self.directory, &self.ssh_key) {
                Ok(repo) => repo,
                Err(e) => panic!("fail git clone. error: {:?}", e),
            },
        };

        if env::set_current_dir(&self.directory).is_err() {
            return;
        }

        // TODO: use git2-rs

        // set branch
        let git_branch = &self.branch;
        debug!("repo: {}, branch: {}", self.url, git_branch);
        let _proc = Command::new("git")
            .arg("checkout")
            .arg(git_branch.to_string())
            .output()
            .expect("fail git checkout command");

        // fetch --prune
        let _proc = Command::new("git")
            .arg("fetch")
            .arg("--prune")
            .output()
            .expect("fail git fetch --prune command");

        // pull
        let _proc = Command::new("git").arg("pull").output().expect("fail git pull command");

        if env::set_current_dir(&old_curdir).is_err() {
            return;
        }
    }

    pub fn collect(self) {
        let old_curdir = env::current_dir().unwrap();

        if env::set_current_dir(&self.directory).is_err() {
            return;
        }
        let dbconn = database::get_database_connection(self.db_url.as_str());

        let mut git_proc = Command::new("git")
            .arg("log")
            .arg("-n 300")
            .arg("--oneline")
            .arg("--date=format:%Y/%m/%d %H:%M:%S")
            .arg("--pretty=format:%D %s\t%cd\t%H")
            .stdout(Stdio::piped())
            .spawn()
            .expect("fail git log command");

        match git_proc.wait() {
            Ok(out) => {
                if !out.success() {
                    panic!("fail git command");
                }
            }
            Err(e) => panic!("fail git command. {:?}", e),
        }

        let mut s = String::new();
        if let Some(ref mut stdout) = git_proc.stdout {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let l = line.unwrap();
                match &self.version_regex {
                    Some(vregex) => {
                        if vregex.is_match(l.as_str()) {
                            s.push_str(l.as_str());
                            s.push_str("\n");
                        }
                    }
                    None => {}
                }
            }
        }

        let reader = BufReader::new(s.trim_end().as_bytes());
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(reader);
        for row in rdr.deserialize() {
            match row {
                Ok(_) => {}
                Err(e) => {
                    error!("fail deserialize csv data. error: {:?}", e);
                    continue;
                }
            };
            let mut record: GitInfo = row.unwrap();
            record.tag = {
                match &self.version_regex {
                    Some(vregex) => match vregex.captures(&record.tag) {
                        Some(caps) => caps.get(1).unwrap().as_str().to_string(),
                        None => "".to_string(),
                    },
                    None => record.tag,
                }
            };
            debug!("record: {:?}", record);
            if record.tag.is_empty() {
                continue;
            }

            let bump_date =
                NaiveDateTime::parse_from_str(record.date.as_str(), "%Y/%m/%d %H:%M:%S").expect("fail parse date");
            let version_history = database::VersionHistory {
                id: 0,
                project_name: self.project_name.clone(),
                channel: self.branch.clone(),
                version: record.tag.clone(),
                bump_date,
                url: Some(format!("{}/releases/tag/{}", self.url, record.tag)),
            };

            match database::insert_version_history(&dbconn, &version_history) {
                Ok(n) => {
                    if n != 0 {
                        info!("insert data. {:?}", version_history);
                    }
                }
                Err(e) => error!("insert error: {:?}", e),
            }
        }

        if env::set_current_dir(&old_curdir).is_err() {
            return;
        }
    }
}
