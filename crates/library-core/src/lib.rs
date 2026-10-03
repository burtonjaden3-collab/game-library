//! Core of Game Library: the data model, the SQLite store, the importers that read
//! each storefront's local data and the online metadata providers. Nothing here
//! depends on Tauri, so it can be tested (and reused by a CLI) without a webview.

pub mod db;
pub mod metadata;
pub mod model;
pub mod steam;

pub use db::Library;
pub use metadata::GameMetadata;
pub use model::{Game, ImportedGame, Source};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A request to an online service failed. `status` is the HTTP status, if one came back.
    #[error("{message}")]
    Http { status: Option<u16>, message: String },
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
