//! Core of Game Library: the data model, the SQLite store and the importers
//! that read each storefront's local data. Nothing here depends on Tauri, so it
//! can be tested (and reused by a CLI) without a webview.

pub mod db;
pub mod model;

pub use db::Library;
pub use model::{Game, ImportedGame, Source};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
