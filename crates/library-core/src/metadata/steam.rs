//! Steam store data for Steam games, from the public `appdetails` endpoint the store
//! page itself uses. No API key needed. Steam rate limits it to roughly 200 requests
//! per five minutes, which is why results are cached.

use serde_json::Value;

use super::{html_to_text, str_field, GameMetadata, Http, MetadataProvider, Rating, Screenshot};
use crate::model::{Game, Source};
use crate::Result;

const APPDETAILS: &str = "https://store.steampowered.com/api/appdetails";

pub struct SteamStore;

impl MetadataProvider for SteamStore {
    fn id(&self) -> &'static str {
        "steam"
    }

    fn supports(&self, game: &Game) -> bool {
        game.source == Source::Steam
    }

    fn fetch(&self, http: &dyn Http, game: &Game) -> Result<Option<GameMetadata>> {
        let app_id = game.source_id.as_str();
        let body = http.get_json(APPDETAILS, &[("appids", app_id), ("l", "english")])?;
        Ok(parse(app_id, &body))
    }
}

/// Parses an `appdetails` response. `None` when Steam has no store page for the app
/// (delisted games, tools, some betas answer `"success": false`).
fn parse(app_id: &str, body: &Value) -> Option<GameMetadata> {
    let entry = body.get(app_id)?;
    if entry.get("success").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let d = entry.get("data")?;

    let names = |key: &str| -> Vec<String> {
        d.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().or_else(|| v.get("description").and_then(Value::as_str)))
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };

    let release = d.get("release_date");
    let release_date = release.and_then(|r| str_field(r, "date")).or_else(|| {
        (release.and_then(|r| r.get("coming_soon")).and_then(Value::as_bool) == Some(true))
            .then(|| "Coming soon".to_owned())
    });

    let platforms = d
        .get("platforms")
        .map(|p| {
            [("linux", "Linux"), ("windows", "Windows"), ("mac", "macOS")]
                .into_iter()
                .filter(|(key, _)| p.get(key).and_then(Value::as_bool) == Some(true))
                .map(|(_, label)| label.to_owned())
                .collect()
        })
        .unwrap_or_default();

    let rating = d.get("metacritic").and_then(|m| {
        let score = m.get("score").and_then(Value::as_u64).filter(|s| *s <= 100)?;
        Some(Rating { score: score as u8, source: "Metacritic".into(), url: str_field(m, "url") })
    });

    let screenshots = d
        .get("screenshots")
        .and_then(Value::as_array)
        .map(|shots| {
            shots
                .iter()
                .filter_map(|s| {
                    Some(Screenshot {
                        thumbnail: str_field(s, "path_thumbnail")?,
                        full: str_field(s, "path_full")?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    let description = str_field(d, "about_the_game")
        .or_else(|| str_field(d, "detailed_description"))
        .map(|html| html_to_text(&html))
        .filter(|s| !s.is_empty());

    Some(GameMetadata {
        provider: "steam".into(),
        source_url: Some(format!("https://store.steampowered.com/app/{app_id}/")),
        summary: str_field(d, "short_description").map(|s| html_to_text(&s)).filter(|s| !s.is_empty()),
        description,
        genres: names("genres"),
        features: names("categories"),
        developers: names("developers"),
        publishers: names("publishers"),
        release_date,
        platforms,
        rating,
        website: str_field(d, "website"),
        cover_url: None,
        screenshots,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::metadata::tests::{game, FakeHttp};

    fn portal2() -> Value {
        json!({
            "620": {
                "success": true,
                "data": {
                    "type": "game",
                    "name": "Portal 2",
                    "steam_appid": 620,
                    "short_description": "The &quot;Perpetual Testing Initiative&quot; has been expanded.",
                    "about_the_game": "<h1>Single Player</h1><p>Portal 2 draws from the award-winning formula.<br><br>Co-op too.</p><ul class=\"bb_ul\"><li>Puzzles</li></ul>",
                    "detailed_description": "unused",
                    "developers": ["Valve"],
                    "publishers": ["Valve", " "],
                    "platforms": { "windows": true, "mac": false, "linux": true },
                    "metacritic": { "score": 95, "url": "https://www.metacritic.com/game/pc/portal-2" },
                    "categories": [{ "id": 2, "description": "Single-player" }, { "id": 9, "description": "Co-op" }],
                    "genres": [{ "id": "1", "description": "Action" }, { "id": "25", "description": "Adventure" }],
                    "screenshots": [
                        { "id": 0, "path_thumbnail": "https://shared.akamai.steamstatic.com/a.600x338.jpg", "path_full": "https://shared.akamai.steamstatic.com/a.1920x1080.jpg" },
                        { "id": 1, "path_thumbnail": "https://shared.akamai.steamstatic.com/b.600x338.jpg" }
                    ],
                    "release_date": { "coming_soon": false, "date": "18 Apr, 2011" },
                    "website": "http://www.thinkwithportals.com/"
                }
            }
        })
    }

    #[test]
    fn parses_store_data() {
        let http = FakeHttp::new([Ok(portal2())]);
        let meta = SteamStore.fetch(&http, &game(Source::Steam, "620", "Portal 2")).unwrap().unwrap();
        assert_eq!(
            http.requests.lock().unwrap()[0],
            r#"GET https://store.steampowered.com/api/appdetails [("appids", "620"), ("l", "english")]"#
        );
        assert_eq!(meta.provider, "steam");
        assert_eq!(meta.source_url.as_deref(), Some("https://store.steampowered.com/app/620/"));
        assert_eq!(meta.summary.as_deref(), Some("The \"Perpetual Testing Initiative\" has been expanded."));
        assert_eq!(
            meta.description.as_deref(),
            Some(
                "Single Player\n\nPortal 2 draws from the award-winning formula.\n\nCo-op too.\n\n• Puzzles"
            )
        );
        assert_eq!(meta.genres, ["Action", "Adventure"]);
        assert_eq!(meta.features, ["Single-player", "Co-op"]);
        assert_eq!(meta.developers, ["Valve"]);
        assert_eq!(meta.publishers, ["Valve"]);
        assert_eq!(meta.platforms, ["Linux", "Windows"]);
        assert_eq!(meta.release_date.as_deref(), Some("18 Apr, 2011"));
        assert_eq!(meta.rating.as_ref().map(|r| r.score), Some(95));
        assert_eq!(meta.website.as_deref(), Some("http://www.thinkwithportals.com/"));
        assert_eq!(meta.screenshots.len(), 1, "screenshots missing a full-size path are skipped");
    }

    #[test]
    fn unknown_app_is_not_found() {
        let http = FakeHttp::new([Ok(json!({ "999": { "success": false } }))]);
        assert_eq!(SteamStore.fetch(&http, &game(Source::Steam, "999", "Gone")).unwrap(), None);
    }

    #[test]
    fn coming_soon_without_date() {
        let mut body = portal2();
        body["620"]["data"]["release_date"] = json!({ "coming_soon": true, "date": "" });
        assert_eq!(parse("620", &body).unwrap().release_date.as_deref(), Some("Coming soon"));
    }

    #[test]
    fn only_steam_games() {
        assert!(SteamStore.supports(&game(Source::Steam, "1", "A")));
        assert!(!SteamStore.supports(&game(Source::Gog, "1", "A")));
    }
}
