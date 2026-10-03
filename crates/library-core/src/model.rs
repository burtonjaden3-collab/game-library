use serde::{Deserialize, Serialize};

/// Where a game came from. Each variant gets its own importer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Gog,
    Itch,
    Manual,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Steam => "steam",
            Source::Epic => "epic",
            Source::Gog => "gog",
            Source::Itch => "itch",
            Source::Manual => "manual",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "steam" => Source::Steam,
            "epic" => Source::Epic,
            "gog" => Source::Gog,
            "itch" => Source::Itch,
            "manual" => Source::Manual,
            _ => return None,
        })
    }
}

/// What an importer reports for one game. The store merges it into a [`Game`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedGame {
    pub source: Source,
    /// The store's own id for the game (a Steam app id, a GOG product id, ...).
    pub source_id: String,
    pub title: String,
    pub install_dir: Option<String>,
    pub installed: bool,
    pub size_bytes: Option<u64>,
    /// Unix seconds.
    pub last_updated: Option<i64>,
    /// Total time played, when the source knows it. `None` keeps what the library has.
    pub playtime_minutes: Option<u64>,
    /// Unix seconds, when the source knows it. `None` keeps what the library has.
    pub last_played: Option<i64>,
}

/// A game as stored in the library and sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub source: Source,
    pub source_id: String,
    pub title: String,
    pub install_dir: Option<String>,
    pub installed: bool,
    pub size_bytes: Option<u64>,
    pub last_updated: Option<i64>,
    /// Unix seconds when the game first entered the library.
    pub added_at: i64,
    pub playtime_minutes: Option<u64>,
    /// Unix seconds.
    pub last_played: Option<i64>,
}
