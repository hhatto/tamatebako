use chrono::NaiveDateTime;
use diesel::insert_or_ignore_into;
use diesel::prelude::*;

mod schema {
    table! {
        version_history {
            id -> Integer,
            project_name -> Text,
            channel -> Text,
            version -> Text,
            bump_date -> Timestamp,
            url -> Nullable<Text>,
        }
    }
}

use self::schema::version_history;

#[derive(Deserialize, Insertable)]
#[table_name = "version_history"]
pub struct VersionHistoryForm {
    project_name: String,
    channel: String,
    version: String,
    url: Option<String>,
}

#[derive(Queryable, PartialEq, Debug)]
pub struct VersionHistory {
    pub id: i32,
    pub project_name: String,
    pub channel: String,
    pub version: String,
    pub bump_date: NaiveDateTime,
    pub url: Option<String>,
}

pub fn get_database_connection(url: &str) -> SqliteConnection {
    SqliteConnection::establish(url).unwrap()
}

pub fn create_table(conn: &SqliteConnection) {
    match conn.execute(
        "CREATE TABLE IF NOT EXISTS version_history (
id INTEGER PRIMARY KEY AUTOINCREMENT,
project_name TEXT,
channel TEXT,
version TEXT,
bump_date TIMESTAMP,
url TEXT,
UNIQUE (project_name, channel, version)
)",
    ) {
        Ok(_) => {}
        Err(e) => error!("create table error. {:?}", e),
    };
}

#[allow(dead_code)]
pub fn have_version_history(
    conn: &SqliteConnection,
    i_name: &str,
    i_channel: &str,
    i_version: &str,
) -> bool {
    use self::schema::version_history::dsl::*;

    match version_history
        .filter(project_name.eq(i_name))
        .filter(channel.eq(i_channel))
        .filter(version.eq(i_version))
        .select(id)
        .count()
        .get_result::<i64>(conn)
    {
        Ok(n) => {
            if n > 0 {
                true
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

pub fn insert_version_history(
    conn: &SqliteConnection,
    input: VersionHistory,
) -> QueryResult<usize> {
    use self::schema::version_history::dsl::*;

    insert_or_ignore_into(version_history)
        .values((
            project_name.eq(input.project_name),
            channel.eq(input.channel),
            version.eq(input.version),
            bump_date.eq(input.bump_date),
            url.eq(input.url),
        )).execute(conn)
}
