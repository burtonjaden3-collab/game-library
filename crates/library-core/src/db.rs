use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::metadata::{CachedMetadata, GameMetadata};
use crate::model::{Game, ImportedGame, Source};
use crate::{Error, Result};

/// Schema migrations, applied in order. `PRAGMA user_version` records how many
/// have run, so append new ones and never edit old ones.
const MIGRATIONS: &[&str] = &[
    r#"
    CREATE TABLE games (
        id           INTEGER PRIMARY KEY,
        source       TEXT    NOT NULL,
        source_id    TEXT    NOT NULL,
        title        TEXT    NOT NULL,
        install_dir  TEXT,
        installed    INTEGER NOT NULL DEFAULT 0,
        size_bytes   INTEGER,
        last_updated INTEGER,
        added_at     INTEGER NOT NULL,
        UNIQUE (source, source_id)
    );
    CREATE INDEX games_title ON games (title COLLATE NOCASE);
"#,
    r#"
    -- One row per game we have looked up. data is GameMetadata as JSON, or NULL when
    -- no provider knew the game (so we don't ask again on every visit).
    CREATE TABLE game_metadata (
        game_id    INTEGER PRIMARY KEY REFERENCES games (id) ON DELETE CASCADE,
        provider   TEXT,
        data       TEXT,
        fetched_at INTEGER NOT NULL
    );
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
"#,
];

/// Counts from merging one importer's results into the library.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeStats {
    pub added: usize,
    pub updated: usize,
    /// Games this source had installed before that it no longer reports.
    /// They stay in the library (you still own them) but are marked uninstalled.
    pub uninstalled: usize,
}

/// The SQLite-backed game library.
pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(dir) = path.as_ref().parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", true)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let tx = conn.transaction()?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", i as i64 + 1)?;
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    pub fn games(&self) -> Result<Vec<Game>> {
        let mut stmt = self.conn.prepare(&format!("{SELECT_GAME} ORDER BY title COLLATE NOCASE"))?;
        let games = stmt.query_map([], game_from_row)?.collect::<rusqlite::Result<_>>()?;
        Ok(games)
    }

    pub fn game(&self, id: i64) -> Result<Option<Game>> {
        Ok(self.conn.query_row(&format!("{SELECT_GAME} WHERE id = ?1"), [id], game_from_row).optional()?)
    }

    /// The cached metadata lookup for a game, if it was ever looked up. An entry whose
    /// JSON no longer parses (written by a different version) counts as never looked up.
    pub fn cached_metadata(&self, game_id: i64) -> Result<Option<CachedMetadata>> {
        let row: Option<(Option<String>, i64)> = self
            .conn
            .query_row("SELECT data, fetched_at FROM game_metadata WHERE game_id = ?1", [game_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        Ok(row.and_then(|(data, fetched_at)| match data {
            None => Some(CachedMetadata { metadata: None, fetched_at }),
            Some(json) => serde_json::from_str(&json)
                .ok()
                .map(|metadata| CachedMetadata { metadata: Some(metadata), fetched_at }),
        }))
    }

    /// Records the result of looking a game up; `None` means no provider knew it.
    pub fn save_metadata(&self, game_id: i64, metadata: Option<&GameMetadata>) -> Result<CachedMetadata> {
        let fetched_at = unix_now();
        let data =
            metadata.map(serde_json::to_string).transpose().map_err(|e| Error::Other(e.to_string()))?;
        self.conn.execute(
            "INSERT INTO game_metadata (game_id, provider, data, fetched_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (game_id) DO UPDATE SET
                provider = excluded.provider, data = excluded.data, fetched_at = excluded.fetched_at",
            params![game_id, metadata.map(|m| m.provider.as_str()), data, fetched_at],
        )?;
        Ok(CachedMetadata { metadata: metadata.cloned(), fetched_at })
    }

    /// Drops every "not found" entry so those games are looked up again, e.g. after a
    /// new provider was switched on.
    pub fn forget_missing_metadata(&self) -> Result<usize> {
        Ok(self.conn.execute("DELETE FROM game_metadata WHERE data IS NULL", [])?)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }

    /// Stores a setting, or removes it when `value` is `None`.
    pub fn set_setting(&self, key: &str, value: Option<&str>) -> Result<()> {
        match value {
            Some(v) => self.conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, v],
            )?,
            None => self.conn.execute("DELETE FROM settings WHERE key = ?1", [key])?,
        };
        Ok(())
    }

    /// Merges one importer's full scan of `source` into the library: new games
    /// are added, known ones refreshed, and installed games missing from the
    /// scan are marked uninstalled. Nothing is ever deleted by an import.
    pub fn merge_import(&mut self, source: Source, scanned: &[ImportedGame]) -> Result<MergeStats> {
        let now = unix_now();
        let mut stats = MergeStats::default();
        let tx = self.conn.transaction()?;
        tx.execute("CREATE TEMP TABLE IF NOT EXISTS seen (source_id TEXT PRIMARY KEY)", [])?;
        tx.execute("DELETE FROM seen", [])?;
        for g in scanned.iter().filter(|g| g.source == source) {
            tx.execute("INSERT OR IGNORE INTO seen (source_id) VALUES (?1)", [&g.source_id])?;
            let existing: Option<i64> = tx
                .query_row(
                    "SELECT id FROM games WHERE source = ?1 AND source_id = ?2",
                    params![source.as_str(), g.source_id],
                    |r| r.get(0),
                )
                .optional()?;
            let size = g.size_bytes.map(|s| s as i64);
            match existing {
                Some(id) => {
                    tx.execute(
                        "UPDATE games SET title = ?1, install_dir = ?2, installed = ?3,
                                size_bytes = ?4, last_updated = ?5 WHERE id = ?6",
                        params![g.title, g.install_dir, g.installed, size, g.last_updated, id],
                    )?;
                    stats.updated += 1;
                }
                None => {
                    tx.execute(
                        "INSERT INTO games (source, source_id, title, install_dir, installed,
                                            size_bytes, last_updated, added_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            source.as_str(),
                            g.source_id,
                            g.title,
                            g.install_dir,
                            g.installed,
                            size,
                            g.last_updated,
                            now
                        ],
                    )?;
                    stats.added += 1;
                }
            }
        }
        stats.uninstalled = tx.execute(
            "UPDATE games SET installed = 0
             WHERE source = ?1 AND installed = 1
               AND source_id NOT IN (SELECT source_id FROM seen)",
            [source.as_str()],
        )?;
        tx.commit()?;
        Ok(stats)
    }
}

const SELECT_GAME: &str = "SELECT id, source, source_id, title, install_dir, installed, size_bytes,
                                  last_updated, added_at
                           FROM games";

fn game_from_row(r: &Row) -> rusqlite::Result<Game> {
    let source: String = r.get(1)?;
    Ok(Game {
        id: r.get(0)?,
        source: Source::parse(&source).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                format!("unknown source {source:?}").into(),
            )
        })?,
        source_id: r.get(2)?,
        title: r.get(3)?,
        install_dir: r.get(4)?,
        installed: r.get(5)?,
        size_bytes: r.get::<_, Option<i64>>(6)?.map(|s| s as u64),
        last_updated: r.get(7)?,
        added_at: r.get(8)?,
    })
}

/// Current Unix time in seconds.
pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam(id: &str, title: &str, installed: bool) -> ImportedGame {
        ImportedGame {
            source: Source::Steam,
            source_id: id.into(),
            title: title.into(),
            install_dir: installed.then(|| format!("/games/{title}")),
            installed,
            size_bytes: Some(1024),
            last_updated: Some(1_700_000_000),
        }
    }

    #[test]
    fn merge_adds_then_updates() {
        let mut lib = Library::open_in_memory().unwrap();
        let stats = lib
            .merge_import(Source::Steam, &[steam("570", "Dota 2", true), steam("440", "TF2", true)])
            .unwrap();
        assert_eq!(stats, MergeStats { added: 2, updated: 0, uninstalled: 0 });

        let stats = lib.merge_import(Source::Steam, &[steam("570", "Dota 2 (renamed)", true)]).unwrap();
        assert_eq!(stats, MergeStats { added: 0, updated: 1, uninstalled: 1 });

        let games = lib.games().unwrap();
        assert_eq!(games.len(), 2, "imports never delete owned games");
        let dota = games.iter().find(|g| g.source_id == "570").unwrap();
        assert_eq!(dota.title, "Dota 2 (renamed)");
        assert!(dota.installed);
        let tf2 = games.iter().find(|g| g.source_id == "440").unwrap();
        assert!(!tf2.installed);
    }

    #[test]
    fn games_are_sorted_case_insensitively() {
        let mut lib = Library::open_in_memory().unwrap();
        lib.merge_import(
            Source::Steam,
            &[steam("1", "beta", true), steam("2", "Alpha", true), steam("3", "Gamma", true)],
        )
        .unwrap();
        let titles: Vec<_> = lib.games().unwrap().into_iter().map(|g| g.title).collect();
        assert_eq!(titles, ["Alpha", "beta", "Gamma"]);
    }

    #[test]
    fn metadata_cache_round_trips() {
        let mut lib = Library::open_in_memory().unwrap();
        lib.merge_import(Source::Steam, &[steam("570", "Dota 2", true), steam("440", "TF2", true)]).unwrap();
        let ids: Vec<i64> = lib.games().unwrap().iter().map(|g| g.id).collect();
        assert_eq!(lib.game(ids[0]).unwrap().unwrap().title, "Dota 2");
        assert_eq!(lib.game(9999).unwrap(), None);
        assert_eq!(lib.cached_metadata(ids[0]).unwrap(), None);

        let meta =
            GameMetadata { provider: "steam".into(), genres: vec!["MOBA".into()], ..Default::default() };
        lib.save_metadata(ids[0], Some(&meta)).unwrap();
        lib.save_metadata(ids[1], None).unwrap();
        assert_eq!(lib.cached_metadata(ids[0]).unwrap().unwrap().metadata, Some(meta.clone()));
        assert_eq!(lib.cached_metadata(ids[1]).unwrap().unwrap().metadata, None);

        // Overwrites, and forgetting misses keeps hits.
        lib.save_metadata(ids[0], Some(&meta)).unwrap();
        assert_eq!(lib.forget_missing_metadata().unwrap(), 1);
        assert_eq!(lib.cached_metadata(ids[1]).unwrap(), None);
        assert!(lib.cached_metadata(ids[0]).unwrap().is_some());
    }

    #[test]
    fn settings_set_and_clear() {
        let lib = Library::open_in_memory().unwrap();
        assert_eq!(lib.setting("k").unwrap(), None);
        lib.set_setting("k", Some("a")).unwrap();
        lib.set_setting("k", Some("b")).unwrap();
        assert_eq!(lib.setting("k").unwrap().as_deref(), Some("b"));
        lib.set_setting("k", None).unwrap();
        assert_eq!(lib.setting("k").unwrap(), None);
    }

    #[test]
    fn reopening_keeps_data_and_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/library.db");
        Library::open(&path).unwrap().merge_import(Source::Steam, &[steam("570", "Dota 2", true)]).unwrap();
        assert_eq!(Library::open(&path).unwrap().games().unwrap().len(), 1);
    }
}
