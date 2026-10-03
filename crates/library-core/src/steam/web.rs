//! Steam Web API: every game a Steam account owns, installed or not, with playtime.
//!
//! Needs the user's own Web API key (from <https://steamcommunity.com/dev/apikey>) and
//! their account, given as a SteamID64, a profile URL or a custom profile name.

use std::collections::HashMap;

use serde_json::Value;

use crate::metadata::{str_field, Http};
use crate::model::{ImportedGame, Source};
use crate::{Error, Result};

const API: &str = "https://api.steampowered.com";

/// Every SteamID64 of an individual account starts with this.
const STEAMID64_PREFIX: &str = "7656119";

/// The user's Steam Web API key and account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub api_key: String,
    pub steam_id: String,
}

/// What the user typed to name their account, before any lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Profile {
    SteamId(String),
    /// A custom URL name (`steamcommunity.com/id/<name>`), resolved through the API.
    Vanity(String),
}

impl Profile {
    /// Accepts a SteamID64, a `steamcommunity.com/profiles/<id>` or `/id/<name>` URL,
    /// or a bare custom URL name.
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.trim().trim_end_matches('/');
        let after =
            |marker: &str| input.split_once(marker).map(|(_, rest)| rest.split('/').next().unwrap_or(""));
        if let Some(id) = after("/profiles/") {
            return is_steam_id(id).then(|| Profile::SteamId(id.to_owned()));
        }
        if let Some(name) = after("/id/") {
            return is_vanity(name).then(|| Profile::Vanity(name.to_owned()));
        }
        if is_steam_id(input) {
            return Some(Profile::SteamId(input.to_owned()));
        }
        (is_vanity(input) && !input.contains('.')).then(|| Profile::Vanity(input.to_owned()))
    }
}

fn is_steam_id(s: &str) -> bool {
    s.len() == 17 && s.starts_with(STEAMID64_PREFIX) && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_vanity(s: &str) -> bool {
    (2..=32).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Turns what the user typed into a SteamID64, asking Steam about custom URL names.
pub fn resolve_profile(http: &dyn Http, api_key: &str, input: &str) -> Result<String> {
    let profile = Profile::parse(input).ok_or_else(|| {
        Error::Other(
            "That doesn't look like a Steam profile. Paste your profile URL or your SteamID64.".into(),
        )
    })?;
    let name = match profile {
        Profile::SteamId(id) => return Ok(id),
        Profile::Vanity(name) => name,
    };
    let resp = call(http, "ISteamUser/ResolveVanityURL/v1/", &[("key", api_key), ("vanityurl", &name)])?;
    let resp = &resp["response"];
    match str_field(resp, "steamid") {
        Some(id) if resp["success"].as_i64() == Some(1) && is_steam_id(&id) => Ok(id),
        _ => Err(Error::Other(format!("Steam has no profile at steamcommunity.com/id/{name}."))),
    }
}

/// One game the account owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGame {
    pub app_id: String,
    pub name: String,
    pub playtime_minutes: u64,
    /// Unix seconds; `None` if never played (or Steam doesn't say).
    pub last_played: Option<i64>,
}

/// Every game the account owns, including free games it has played.
pub fn owned_games(http: &dyn Http, account: &Account) -> Result<Vec<OwnedGame>> {
    let resp = call(
        http,
        "IPlayerService/GetOwnedGames/v1/",
        &[
            ("key", &account.api_key),
            ("steamid", &account.steam_id),
            ("include_appinfo", "1"),
            ("include_played_free_games", "1"),
        ],
    )?;
    // A private "Game details" setting gets an empty response rather than an error.
    let Some(games) = resp["response"]["games"].as_array() else {
        return Err(Error::Other(
            "Steam didn't list any games for this account. If you own games, set \"Game details\" \
             to Public in your Steam profile's privacy settings."
                .into(),
        ));
    };
    Ok(games
        .iter()
        .filter_map(|g| {
            let app_id = g["appid"].as_u64()?.to_string();
            let name = str_field(g, "name").unwrap_or_else(|| format!("Steam app {app_id}"));
            Some(OwnedGame {
                app_id,
                name,
                playtime_minutes: g["playtime_forever"].as_u64().unwrap_or(0),
                last_played: g["rtime_last_played"].as_i64().filter(|&t| t > 0),
            })
        })
        .collect())
}

fn call(http: &dyn Http, path: &str, query: &[(&str, &str)]) -> Result<Value> {
    http.get_json(&format!("{API}/{path}"), query).map_err(|e| match e {
        Error::Http { status: Some(401 | 403), .. } => {
            Error::Other("Steam rejected the Web API key. Check it at steamcommunity.com/dev/apikey.".into())
        }
        other => other,
    })
}

/// Combines the local scan with the account's owned games: installed games keep what
/// their manifests say and gain playtime, and owned games that aren't on disk are added
/// as not installed. Local games the account doesn't own (Family Sharing) are kept.
pub fn with_owned_games(local: Vec<ImportedGame>, owned: Vec<OwnedGame>) -> Vec<ImportedGame> {
    let mut owned: HashMap<String, OwnedGame> = owned.into_iter().map(|g| (g.app_id.clone(), g)).collect();
    let mut games: Vec<ImportedGame> = local
        .into_iter()
        .map(|mut g| {
            if let Some(o) = owned.remove(&g.source_id) {
                g.playtime_minutes = Some(o.playtime_minutes);
                g.last_played = o.last_played;
            }
            g
        })
        .collect();
    let mut rest: Vec<OwnedGame> = owned.into_values().collect();
    rest.sort_by(|a, b| a.app_id.cmp(&b.app_id));
    games.extend(rest.into_iter().map(|o| ImportedGame {
        source: Source::Steam,
        source_id: o.app_id,
        title: o.name,
        install_dir: None,
        installed: false,
        size_bytes: None,
        last_updated: None,
        playtime_minutes: Some(o.playtime_minutes),
        last_played: o.last_played,
    }));
    games
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::metadata::tests::FakeHttp;

    const ID: &str = "76561197960287930";

    fn account() -> Account {
        Account { api_key: "KEY".into(), steam_id: ID.into() }
    }

    #[test]
    fn parses_every_way_of_naming_a_profile() {
        let id = Some(Profile::SteamId(ID.into()));
        let vanity = Some(Profile::Vanity("gabelogannewell".into()));
        assert_eq!(Profile::parse(ID), id);
        assert_eq!(Profile::parse(&format!(" https://steamcommunity.com/profiles/{ID}/ ")), id);
        assert_eq!(Profile::parse(&format!("steamcommunity.com/profiles/{ID}/games")), id);
        assert_eq!(Profile::parse("https://steamcommunity.com/id/gabelogannewell/"), vanity);
        assert_eq!(Profile::parse("gabelogannewell"), vanity);
        assert_eq!(Profile::parse("https://steamcommunity.com/profiles/123"), None);
        assert_eq!(Profile::parse("https://example.com"), None);
        assert_eq!(Profile::parse("two words"), None);
        assert_eq!(Profile::parse(""), None);
    }

    #[test]
    fn resolves_custom_urls_through_the_api() {
        let http = FakeHttp::new([
            Ok(json!({ "response": { "steamid": ID, "success": 1 } })),
            Ok(json!({ "response": { "success": 42, "message": "No match" } })),
        ]);
        assert_eq!(resolve_profile(&http, "KEY", "steamcommunity.com/id/gaben").unwrap(), ID);
        let err = resolve_profile(&http, "KEY", "nobody-here").unwrap_err();
        assert!(err.to_string().contains("/id/nobody-here"), "{err}");
        // SteamID64s need no lookup.
        assert_eq!(resolve_profile(&http, "KEY", ID).unwrap(), ID);
        let requests = http.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].contains("ResolveVanityURL") && requests[0].contains("\"gaben\""));
    }

    #[test]
    fn reads_owned_games_with_playtime() {
        let http = FakeHttp::new([Ok(json!({ "response": { "game_count": 2, "games": [
            { "appid": 620, "name": "Portal 2", "playtime_forever": 754, "rtime_last_played": 1_700_000_000 },
            { "appid": 400, "name": "Portal", "playtime_forever": 0, "rtime_last_played": 0 },
            { "name": "no appid" }
        ] } }))]);
        let games = owned_games(&http, &account()).unwrap();
        assert_eq!(
            games,
            [
                OwnedGame {
                    app_id: "620".into(),
                    name: "Portal 2".into(),
                    playtime_minutes: 754,
                    last_played: Some(1_700_000_000)
                },
                OwnedGame {
                    app_id: "400".into(),
                    name: "Portal".into(),
                    playtime_minutes: 0,
                    last_played: None
                },
            ]
        );
        let request = &http.requests.lock().unwrap()[0];
        assert!(
            request.contains("GetOwnedGames") && request.contains(ID) && request.contains("include_appinfo")
        );
    }

    #[test]
    fn explains_private_profiles_and_bad_keys() {
        let http = FakeHttp::new([
            Ok(json!({ "response": {} })),
            Err(Error::Http { status: Some(403), message: "forbidden".into() }),
        ]);
        assert!(owned_games(&http, &account()).unwrap_err().to_string().contains("Game details"));
        assert!(owned_games(&http, &account()).unwrap_err().to_string().contains("Web API key"));
    }

    #[test]
    fn owned_games_join_the_local_scan() {
        let local = |id: &str, title: &str| ImportedGame {
            source: Source::Steam,
            source_id: id.into(),
            title: title.into(),
            install_dir: Some(format!("/lib/{title}")),
            installed: true,
            size_bytes: Some(1),
            last_updated: Some(1),
            playtime_minutes: None,
            last_played: None,
        };
        let owned = |id: &str, name: &str, minutes: u64| OwnedGame {
            app_id: id.into(),
            name: name.into(),
            playtime_minutes: minutes,
            last_played: None,
        };
        let games = with_owned_games(
            vec![local("620", "Portal 2"), local("570", "Shared Game")],
            vec![owned("620", "Portal 2 (API)", 60), owned("440", "TF2", 5), owned("400", "Portal", 0)],
        );
        let summary: Vec<_> = games
            .iter()
            .map(|g| (g.source_id.as_str(), g.title.as_str(), g.installed, g.playtime_minutes))
            .collect();
        assert_eq!(
            summary,
            [
                ("620", "Portal 2", true, Some(60)),
                ("570", "Shared Game", true, None),
                ("400", "Portal", false, Some(0)),
                ("440", "TF2", false, Some(5)),
            ]
        );
        assert_eq!(games[0].install_dir.as_deref(), Some("/lib/Portal 2"));
    }
}
