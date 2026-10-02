use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::model::{Game, ImportedGame, Source};
use crate::Result;

/// Schema migrations, applied in order. `PRAGMA user_version` records how many
/// have run, so append new ones and never edit old ones.
const MIGRATIONS: &[&str] = &[r#"
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
"#];

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
        let mut stmt = self.conn.prepare(
            "SELECT id, source, source_id, title, install_dir, installed, size_bytes,
                    last_updated, added_at
             FROM games ORDER BY title COLLATE NOCASE",
        )?;
        let games = stmt.query_map([], game_from_row)?.collect::<rusqlite::Result<_>>()?;
        Ok(games)
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

fn unix_now() -> i64 {
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
    fn reopening_keeps_data_and_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/library.db");
        Library::open(&path).unwrap().merge_import(Source::Steam, &[steam("570", "Dota 2", true)]).unwrap();
        assert_eq!(Library::open(&path).unwrap().games().unwrap().len(), 1);
    }
}
