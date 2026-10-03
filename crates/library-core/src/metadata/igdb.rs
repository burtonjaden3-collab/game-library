//! IGDB (Twitch's game database) for games from any store, including ones Steam's store
//! doesn't list. IGDB needs a Twitch application's client ID and secret; users register
//! their own free app at https://dev.twitch.tv/console/apps and paste the pair into
//! settings. Nothing ships with the app.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{format_unix_date, str_field, GameMetadata, Http, MetadataProvider, Rating, Screenshot};
use crate::model::Game;
use crate::{Error, Result};

const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const GAMES_URL: &str = "https://api.igdb.com/v4/games";
const IMAGE_URL: &str = "https://images.igdb.com/igdb/image/upload";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

pub struct Igdb {
    credentials: Credentials,
    token: Mutex<Option<(String, Instant)>>,
}

impl Igdb {
    pub fn new(credentials: Credentials) -> Self {
        Self { credentials, token: Mutex::new(None) }
    }

    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    /// Fetches an app access token, proving the credentials work.
    pub fn check(&self, http: &dyn Http) -> Result<()> {
        self.token(http).map(drop)
    }

    fn token(&self, http: &dyn Http) -> Result<String> {
        let mut cached = self.token.lock().map_err(|e| Error::Other(e.to_string()))?;
        if let Some((token, expires)) = cached.as_ref() {
            if Instant::now() < *expires {
                return Ok(token.clone());
            }
        }
        let body = http
            .post_json(
                TOKEN_URL,
                &[
                    ("client_id", &self.credentials.client_id),
                    ("client_secret", &self.credentials.client_secret),
                    ("grant_type", "client_credentials"),
                ],
                &[],
                "",
            )
            .map_err(|e| match e {
                Error::Http { status: Some(400 | 401 | 403), .. } => Error::Http {
                    status: None,
                    message: "Twitch rejected the IGDB client ID or secret".into(),
                },
                other => other,
            })?;
        let token = str_field(&body, "access_token")
            .ok_or_else(|| Error::Other("Twitch's token response had no access_token".into()))?;
        let lifetime = body.get("expires_in").and_then(Value::as_u64).unwrap_or(3600);
        let expires = Instant::now() + Duration::from_secs(lifetime.saturating_sub(60));
        *cached = Some((token.clone(), expires));
        Ok(token)
    }

    fn search(&self, http: &dyn Http, title: &str) -> Result<Value> {
        let escaped = title.replace('\\', "\\\\").replace('"', "\\\"");
        let query = format!(
            "search \"{escaped}\"; \
             fields name,url,summary,storyline,first_release_date,genres.name,themes.name,game_modes.name,\
             platforms.name,involved_companies.company.name,involved_companies.developer,\
             involved_companies.publisher,aggregated_rating,total_rating,cover.image_id,\
             screenshots.image_id; \
             where version_parent = null; limit 10;"
        );
        let mut retried = false;
        loop {
            let token = self.token(http)?;
            let auth = format!("Bearer {token}");
            let headers =
                [("Client-ID", self.credentials.client_id.as_str()), ("Authorization", auth.as_str())];
            match http.post_json(GAMES_URL, &[], &headers, &query) {
                // The token was revoked or expired early: get a new one, once.
                Err(Error::Http { status: Some(401), .. }) if !retried => {
                    retried = true;
                    *self.token.lock().map_err(|e| Error::Other(e.to_string()))? = None;
                }
                other => return other,
            }
        }
    }
}

impl MetadataProvider for Igdb {
    fn id(&self) -> &'static str {
        "igdb"
    }

    fn supports(&self, _: &Game) -> bool {
        true
    }

    fn fetch(&self, http: &dyn Http, game: &Game) -> Result<Option<GameMetadata>> {
        let results = self.search(http, &game.title)?;
        Ok(best_match(&game.title, &results).map(parse))
    }
}

/// Lowercase letters and digits only, so "Half-Life 2™" matches "Half Life 2".
fn normalize(title: &str) -> String {
    title.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// An exact (normalized) title match, else the first result whose title contains ours
/// or is contained in it. Anything looser risks showing the wrong game.
fn best_match<'a>(title: &str, results: &'a Value) -> Option<&'a Value> {
    let wanted = normalize(title);
    if wanted.is_empty() {
        return None;
    }
    let results = results.as_array()?;
    let name = |v: &Value| normalize(v.get("name").and_then(Value::as_str).unwrap_or_default());
    results.iter().find(|v| name(v) == wanted).or_else(|| {
        results.iter().find(|v| {
            let n = name(v);
            !n.is_empty() && (n.contains(&wanted) || wanted.contains(&n))
        })
    })
}

fn names(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(|i| str_field(i, "name")).collect())
        .unwrap_or_default()
}

fn image(image_id: &str, size: &str) -> String {
    format!("{IMAGE_URL}/{size}/{image_id}.jpg")
}

fn parse(g: &Value) -> GameMetadata {
    let companies = |role: &str| -> Vec<String> {
        g.get("involved_companies")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|c| c.get(role).and_then(Value::as_bool) == Some(true))
                    .filter_map(|c| c.get("company").and_then(|co| str_field(co, "name")))
                    .collect()
            })
            .unwrap_or_default()
    };

    let rating = [("aggregated_rating", "IGDB critics"), ("total_rating", "IGDB")].into_iter().find_map(
        |(key, source)| {
            let score = g.get(key).and_then(Value::as_f64).filter(|s| (0.0..=100.0).contains(s))?;
            Some(Rating { score: score.round() as u8, source: source.into(), url: str_field(g, "url") })
        },
    );

    let mut genres = names(g, "genres");
    genres.extend(names(g, "themes"));

    GameMetadata {
        provider: "igdb".into(),
        source_url: str_field(g, "url"),
        summary: str_field(g, "summary"),
        description: str_field(g, "storyline"),
        genres,
        features: names(g, "game_modes"),
        developers: companies("developer"),
        publishers: companies("publisher"),
        release_date: g.get("first_release_date").and_then(Value::as_i64).map(format_unix_date),
        platforms: names(g, "platforms"),
        rating,
        website: None,
        cover_url: g.get("cover").and_then(|c| str_field(c, "image_id")).map(|id| image(&id, "t_cover_big")),
        screenshots: g
            .get("screenshots")
            .and_then(Value::as_array)
            .map(|shots| {
                shots
                    .iter()
                    .filter_map(|s| str_field(s, "image_id"))
                    .map(|id| Screenshot {
                        thumbnail: image(&id, "t_screenshot_med"),
                        full: image(&id, "t_1080p"),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::metadata::tests::{game, FakeHttp};
    use crate::Source;

    fn igdb() -> Igdb {
        Igdb::new(Credentials { client_id: "id".into(), client_secret: "secret".into() })
    }

    fn token() -> Result<Value> {
        Ok(json!({ "access_token": "tok", "expires_in": 5000000, "token_type": "bearer" }))
    }

    fn hades() -> Value {
        json!([
            { "id": 1, "name": "Hades II", "url": "https://www.igdb.com/games/hades-ii" },
            {
                "id": 113112,
                "name": "Hades",
                "url": "https://www.igdb.com/games/hades--1",
                "summary": "Defy the god of the dead.",
                "first_release_date": 1600387200,
                "genres": [{ "id": 12, "name": "Role-playing (RPG)" }],
                "themes": [{ "id": 1, "name": "Action" }],
                "game_modes": [{ "id": 1, "name": "Single player" }],
                "platforms": [{ "id": 6, "name": "PC (Microsoft Windows)" }],
                "involved_companies": [
                    { "company": { "name": "Supergiant Games" }, "developer": true, "publisher": true },
                    { "company": { "name": "Private Division" }, "developer": false, "publisher": true }
                ],
                "aggregated_rating": 93.4,
                "total_rating": 90.0,
                "cover": { "image_id": "co39vc" },
                "screenshots": [{ "image_id": "sc1" }]
            }
        ])
    }

    #[test]
    fn parses_the_exact_title_match() {
        let http = FakeHttp::new([token(), Ok(hades())]);
        let meta = igdb().fetch(&http, &game(Source::Epic, "x", "HADES™")).unwrap().unwrap();
        assert_eq!(meta.provider, "igdb");
        assert_eq!(meta.summary.as_deref(), Some("Defy the god of the dead."));
        assert_eq!(meta.genres, ["Role-playing (RPG)", "Action"]);
        assert_eq!(meta.features, ["Single player"]);
        assert_eq!(meta.developers, ["Supergiant Games"]);
        assert_eq!(meta.publishers, ["Supergiant Games", "Private Division"]);
        assert_eq!(meta.release_date.as_deref(), Some("18 Sep 2020"));
        assert_eq!(meta.rating.as_ref().map(|r| (r.score, r.source.as_str())), Some((93, "IGDB critics")));
        assert_eq!(
            meta.cover_url.as_deref(),
            Some("https://images.igdb.com/igdb/image/upload/t_cover_big/co39vc.jpg")
        );
        assert_eq!(meta.screenshots[0].full, "https://images.igdb.com/igdb/image/upload/t_1080p/sc1.jpg");

        let requests = http.requests.lock().unwrap();
        assert!(requests[0].starts_with("POST https://id.twitch.tv/oauth2/token"));
        assert!(requests[1].contains(r#"search "HADES™";"#), "{}", requests[1]);
    }

    #[test]
    fn reuses_the_token_and_escapes_quotes() {
        let http = FakeHttp::new([token(), Ok(json!([])), Ok(json!([]))]);
        let p = igdb();
        assert_eq!(p.fetch(&http, &game(Source::Gog, "x", "Nope")).unwrap(), None);
        assert_eq!(p.fetch(&http, &game(Source::Gog, "x", r#"Say "hi""#)).unwrap(), None);
        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), 3, "one token request for both searches");
        assert!(requests[2].contains(r#"search "Say \"hi\"";"#), "{}", requests[2]);
    }

    #[test]
    fn retries_once_with_a_fresh_token_on_401() {
        let unauthorized = || Err(Error::Http { status: Some(401), message: String::new() });
        let http = FakeHttp::new([token(), unauthorized(), token(), Ok(hades())]);
        assert!(igdb().fetch(&http, &game(Source::Gog, "x", "Hades")).unwrap().is_some());
    }

    #[test]
    fn bad_credentials_are_reported_plainly() {
        let http = FakeHttp::new([Err(Error::Http { status: Some(400), message: "raw".into() })]);
        let err = igdb().check(&http).unwrap_err().to_string();
        assert!(err.contains("rejected the IGDB client ID or secret"), "{err}");
    }

    #[test]
    fn matching_is_strict_enough_to_avoid_wrong_games() {
        let results = json!([{ "name": "Portal 2" }, { "name": "Portal" }]);
        assert_eq!(best_match("portal", &results).unwrap()["name"], "Portal");
        assert_eq!(best_match("Portal 2: Deluxe", &results).unwrap()["name"], "Portal 2");
        assert!(best_match("Celeste", &results).is_none());
        assert!(best_match("™", &results).is_none());
    }
}
