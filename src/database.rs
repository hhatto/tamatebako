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
pub fn have_version_history(conn: &SqliteConnection, i_name: &str, i_channel: &str, i_version: &str) -> bool {
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

pub fn insert_version_history(conn: &SqliteConnection, input: &VersionHistory) -> QueryResult<usize> {
    use self::schema::version_history::dsl::*;

    insert_or_ignore_into(version_history)
        .values((
            project_name.eq(input.project_name.clone()),
            channel.eq(input.channel.clone()),
            version.eq(input.version.clone()),
            bump_date.eq(input.bump_date.clone()),
            url.eq(input.url.clone()),
        ))
        .execute(conn)
}

pub fn get_latest_version_history(conn: &SqliteConnection, order_by: Option<String>, is_order_by_desc: bool) -> Vec<VersionHistory> {
    use self::schema::version_history::dsl::*;

    let mut query = version_history.into_boxed();
    query = query.group_by(project_name);
    // TODO: not elegant code...
    query = match order_by {
        Some(v) => {
            if v == "version" {
                if is_order_by_desc {
                    query.order(version.desc())
                } else {
                    query.order(version.asc())
                }
            } else if v == "bump_date" {
                if is_order_by_desc {
                    query.order(bump_date.desc())
                } else {
                    query.order(bump_date.asc())
                }
            } else {
                if is_order_by_desc {
                    query.order(project_name.desc())
                } else {
                    query.order(project_name.asc())
                }
            }
        }
        None => if is_order_by_desc {
            query.order(project_name.desc())
        } else {
            query.order(project_name.asc())
        }
    };
    let version_histories = query.load::<VersionHistory>(conn).unwrap();

    let mut ret: Vec<VersionHistory> = vec![];
    for v in &version_histories {
        ret.push(
            version_history
                .filter(project_name.eq(v.project_name.to_string()))
                .order(bump_date.desc())
                .limit(1)
                .first::<VersionHistory>(conn)
                .unwrap(),
        );
    }
    ret
}

pub fn get_version_history(conn: &SqliteConnection) -> Vec<VersionHistory> {
    use self::schema::version_history::dsl::*;

    //version_history
    //    .select(())
    //    .get_result(conn)
    version_history
        .order(bump_date.desc())
        .load::<VersionHistory>(conn)
        .unwrap()
}
