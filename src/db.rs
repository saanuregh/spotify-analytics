use chrono::{DateTime, Utc};
use color_eyre::eyre::{Context, Result};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::path::Path;
use tracing::{info, instrument};


fn get_db() -> Result<Connection> {
    let migrations = Migrations::new(vec![M::up(
        "CREATE TABLE spotify_history (
            ts DATETIME NOT NULL,
            username TEXT,
            platform TEXT,
            ms_played UNSIGNED BIG INT,
            conn_country TEXT,
            ip_addr_decrypted TEXT,
            user_agent_decrypted TEXT,
            master_metadata_track_name TEXT,
            master_metadata_album_artist_name TEXT,
            master_metadata_album_album_name TEXT,
            spotify_track_uri TEXT,
            episode_name TEXT,
            episode_show_name TEXT,
            spotify_episode_uri TEXT,
            reason_start TEXT,
            reason_end TEXT,
            shuffle BOOLEAN,
            skipped BOOLEAN,
            offline BOOLEAN,
            offline_timestamp UNSIGNED BIG INT,
            incognito_mode BOOLEAN
          );",
    )
    .down("DROP TABLE spotify_history;")]);

    let mut conn = Connection::open("./spotify_history.db")?;
    conn.pragma_update(None, "journal_mode", &"WAL")?;

    migrations.to_latest(&mut conn)?;

    Ok(conn)
}

pub struct SpotifyAnalytics {
    history: Vec<SpotifyHistoryEntry>,
    max_ts: DateTime<Utc>,
    min_ts: DateTime<Utc>,
}

impl SpotifyAnalytics {
    pub fn new() -> Result<Self> {
        let conn = get_db()?;
        let mut stmt = conn.prepare("SELECT * FROM spotify_history")?;
        let history: Vec<SpotifyHistoryEntry> =
            serde_rusqlite::from_rows::<SpotifyHistoryEntry>(stmt.query([])?)
                .collect::<Result<_, serde_rusqlite::Error>>()?;
        let max_ts = history
            .iter()
            .max_by_key(|x| x.ts)
            .map(|x| x.ts)
            .unwrap_or(DateTime::<Utc>::MIN_UTC);
        let min_ts = history
            .iter()
            .min_by_key(|x| x.ts)
            .map(|x| x.ts)
            .unwrap_or(DateTime::<Utc>::MAX_UTC);
        Ok(Self {
            history,
            max_ts,
            min_ts,
        })
    }

    #[instrument(skip(self), err)]
    pub fn deserialize_extended_streaming_history_json<P>(&mut self, path: P) -> Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        let data = fs::read(path)?;
        let history: Vec<SpotifyHistoryEntry> = serde_json::from_slice(&data)?;
        self.history.extend(history);
        Ok(())
    }

    #[instrument(skip(self), err)]
    pub fn deserialize_extended_streaming_history_json_files_from_folder<P>(
        &mut self,
        dir_path: P,
    ) -> Result<()>
    where
        P: AsRef<Path> + Debug,
    {
        for dir_entry in fs::read_dir(dir_path)? {
            let dir_entry = dir_entry?;
            let path = dir_entry.path();

            if !dir_entry.file_type()?.is_file() {
                info!(?path, "ignoring dir");
                continue;
            }

            if let Some(ext) = path.extension() {
                if ext != "json" {
                    info!(?path, "ignoring non-json file");
                    continue;
                }
            }

            self.deserialize_extended_streaming_history_json(path)?
        }
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let conn = get_db()?;

        let mut stmt = conn.prepare_cached(
            "INSERT INTO spotify_history VALUES (
            :ts,
            :username,
            :platform,
            :ms_played,
            :conn_country,
            :ip_addr_decrypted,
            :user_agent_decrypted,
            :master_metadata_track_name,
            :master_metadata_album_artist_name,
            :master_metadata_album_album_name,
            :spotify_track_uri,
            :episode_name,
            :episode_show_name,
            :spotify_episode_uri,
            :reason_start,
            :reason_end,
            :shuffle,
            :skipped,
            :offline,
            :offline_timestamp,
            :incognito_mode
          );",
        )?;
        for e in self
            .history
            .iter()
            .filter(move |x| x.ts > self.max_ts && x.ts < self.min_ts)
        {
            let p = serde_rusqlite::to_params_named(e)?;
            stmt.execute(p.to_slice().as_slice())
                .with_context(|| format!("{:?}", e))?;
        }
        Ok(())
    }

    pub fn get_all_top_artists(&self) -> Vec<(&str, u64)> {
        let mut s = HashMap::new();
        for x in self.history.iter() {
            if let Some(a) = x.master_metadata_album_artist_name.as_ref() {
                let p = s.entry(a.as_str()).or_insert(0_u64);
                *p = p.saturating_add(x.ms_played);
            }
        }
        let mut r: Vec<(&str, u64)> = s.into_iter().collect();
        r.sort_by(|a, b| b.1.cmp(&a.1));
        r
    }

    pub fn get_top_10_artists(&self) -> Vec<(&str, u64)> {
        self.get_all_top_artists().into_iter().take(10).collect()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpotifyHistoryEntry {
    pub ts: DateTime<Utc>,
    pub username: Option<String>,
    pub platform: Option<String>,
    pub ms_played: u64,
    pub conn_country: Option<String>,
    pub ip_addr_decrypted: Option<String>,
    pub user_agent_decrypted: Option<String>,
    pub master_metadata_track_name: Option<String>,
    pub master_metadata_album_artist_name: Option<String>,
    pub master_metadata_album_album_name: Option<String>,
    pub spotify_track_uri: Option<String>,
    pub episode_name: Option<String>,
    pub episode_show_name: Option<String>,
    pub spotify_episode_uri: Option<String>,
    pub reason_start: Option<String>,
    pub reason_end: Option<String>,
    pub shuffle: Option<bool>,
    pub skipped: Option<bool>,
    pub offline: Option<bool>,
    pub offline_timestamp: Option<u64>,
    pub incognito_mode: Option<bool>,
}
