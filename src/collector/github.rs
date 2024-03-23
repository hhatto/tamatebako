use chrono::NaiveDateTime;
use reqwest;
use reqwest::Client;
use url::Url;

use crate::database;

const GITHUB_API: &str = "https://api.github.com";

#[derive(Debug, Serialize, Deserialize)]
struct GitHubRelease {
    html_url: String,
    tag_name: String,
    created_at: String,
}

pub struct GitHubCollector {
    db_url: String,
    project_name: String,
    client: Client,
    owner: String,
    repo_name: String,
    access_token: Option<String>,
}

impl GitHubCollector {
    pub fn new(db_url: &str, project_name: &str, owner: &str, repo_name: &str, access_token: Option<String>) -> Self {
        Self {
            db_url: db_url.to_string(),
            project_name: project_name.to_string(),
            client: Client::new(),
            owner: owner.to_string(),
            repo_name: repo_name.to_string(),
            access_token,
        }
    }

    fn insert(&self, tag: &str, date: &str, release_url: &str) -> usize {
        let mut dbconn = database::get_database_connection(self.db_url.as_str());
        let bump_date = NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%SZ").expect("fail parse date");
        let version_history = database::VersionHistory {
            id: 0,
            project_name: self.project_name.clone(),
            channel: "".to_string(),
            version: tag.to_string(),
            bump_date,
            url: Some(release_url.to_string()),
        };

        match database::insert_version_history(&mut dbconn, &version_history) {
            Ok(n) => {
                if n != 0 {
                    info!("insert data. {:?}", version_history);
                }
                n
            }
            Err(e) => {
                error!("insert error: {:?}", e);
                0
            }
        }
    }

    pub async fn get_releases(self) -> Result<usize, reqwest::Error> {
        debug!("get_releases");
        let url = Url::parse(GITHUB_API).unwrap();
        let url_path = format!("repos/{}/{}/releases", self.owner, self.repo_name);
        let mut get_url = url.join(url_path.as_str()).unwrap();
        let res: Vec<GitHubRelease> = match &self.access_token {
            Some(token) => {
                let t = format!("access_token={}", token);
                get_url.set_query(Some(t.as_str()));
                self.client
                    .get(get_url.as_str())
                    .header("user-agent", "tamatebako-client")
                    .send()
                    .await?
                    .json()
                    .await?
            }
            None => {
                self.client
                    .get(get_url.as_str())
                    .header("user-agent", "tamatebako-client")
                    .send()
                    .await?
                    .json()
                    .await?
            }
        };

        debug!("github.release: {:#?}", res);
        let mut all_insert_num = 0;
        for release in res.iter() {
            let release_url = release.html_url.as_str();
            let tag_name = release.tag_name.as_str();
            let bump_date = release.created_at.as_str();

            let insert_num = self.insert(tag_name, bump_date, release_url);
            all_insert_num += insert_num;
        }
        Ok(all_insert_num)
    }
}
