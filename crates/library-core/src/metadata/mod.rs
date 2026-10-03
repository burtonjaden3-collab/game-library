//! Game metadata (descriptions, genres, developers, release dates, screenshots, ...) from
//! online providers. Each provider knows which games it can describe; [`fetch`] asks them
//! in order and keeps the first match. Results are cached in the library database by the
//! caller, so providers are only hit when a game has no fresh cached entry.

pub mod igdb;
pub mod steam;

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Game;
use crate::{Error, Result};

/// Everything we know about a game beyond what its store keeps on disk.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameMetadata {
    /// Which provider this came from, e.g. "steam" or "igdb".
    pub provider: String,
    /// The provider's page for the game.
    pub source_url: Option<String>,
    /// One or two sentences.
    pub summary: Option<String>,
    /// The long description as plain text, paragraphs separated by blank lines.
    pub description: Option<String>,
    pub genres: Vec<String>,
    /// Modes and store features such as "Single-player" or "Steam Cloud".
    pub features: Vec<String>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    /// Human-readable, as the provider reports it (e.g. "9 Jul, 2013" or "Coming soon").
    pub release_date: Option<String>,
    pub platforms: Vec<String>,
    pub rating: Option<Rating>,
    pub website: Option<String>,
    pub cover_url: Option<String>,
    pub screenshots: Vec<Screenshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rating {
    /// 0 to 100.
    pub score: u8,
    /// Who gave it, e.g. "Metacritic" or "IGDB critics".
    pub source: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub thumbnail: String,
    pub full: String,
}

/// The outcome of the last lookup for a game, as cached in the library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedMetadata {
    /// `None` when no provider knew the game.
    pub metadata: Option<GameMetadata>,
    /// Unix seconds.
    pub fetched_at: i64,
}

impl CachedMetadata {
    /// Hits are kept for 30 days; misses are retried after 3, since stores add pages.
    pub fn is_fresh(&self, now: i64) -> bool {
        let max_age = if self.metadata.is_some() { 30 } else { 3 } * 86_400;
        now - self.fetched_at < max_age
    }
}

/// A source of game metadata.
pub trait MetadataProvider: Send + Sync {
    /// Short stable id stored alongside cached results.
    fn id(&self) -> &'static str;
    /// Whether this provider can look up `game` at all.
    fn supports(&self, game: &Game) -> bool;
    /// `Ok(None)` means the provider answered and has nothing for this game.
    fn fetch(&self, http: &dyn Http, game: &Game) -> Result<Option<GameMetadata>>;
}

/// Asks each provider that supports `game` in turn and returns the first match.
///
/// `Ok(None)` means every provider answered and none knew the game. If any provider
/// failed (network down, rate limited) and none matched, that error is returned instead,
/// so callers don't cache a "not found" that was really an outage.
pub fn fetch(
    providers: &[&dyn MetadataProvider],
    http: &dyn Http,
    game: &Game,
) -> Result<Option<GameMetadata>> {
    let mut failure = None;
    for provider in providers.iter().filter(|p| p.supports(game)) {
        match provider.fetch(http, game) {
            Ok(Some(meta)) => return Ok(Some(meta)),
            Ok(None) => {}
            Err(e) => failure = Some(e),
        }
    }
    failure.map_or(Ok(None), Err)
}

/// The HTTP calls providers make. A trait so tests can stand in canned responses.
pub trait Http: Send + Sync {
    fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value>;
    fn post_json(
        &self,
        url: &str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Value>;
}

/// The real HTTP client.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl Default for UreqHttp {
    fn default() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(20)))
            .http_status_as_error(false)
            .user_agent(concat!("GameLibrary/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { agent }
    }
}

impl UreqHttp {
    fn finish(
        url: &str,
        result: std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
    ) -> Result<Value> {
        let host = url.split('/').nth(2).unwrap_or(url);
        let mut resp = result.map_err(|e| Error::Http { status: None, message: format!("{host}: {e}") })?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Http { status: Some(status), message: format!("{host}: {e}") })?;
        if !(200..300).contains(&status) {
            let message = match status {
                429 => format!("{host} is rate limiting requests; try again in a few minutes"),
                _ => format!("{host} answered {status}: {}", text.chars().take(200).collect::<String>()),
            };
            return Err(Error::Http { status: Some(status), message });
        }
        serde_json::from_str(&text)
            .map_err(|e| Error::Http { status: Some(status), message: format!("{host}: bad JSON: {e}") })
    }
}

impl Http for UreqHttp {
    fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value> {
        let mut req = self.agent.get(url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        Self::finish(url, req.call())
    }

    fn post_json(
        &self,
        url: &str,
        query: &[(&str, &str)],
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<Value> {
        let mut req = self.agent.post(url);
        for (k, v) in query {
            req = req.query(*k, *v);
        }
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        Self::finish(url, req.send(body))
    }
}

/// Turns store-page HTML into plain text: block tags become line breaks, list items get
/// bullets, other tags are dropped and common entities decoded. Never renders HTML, so
/// nothing from a store page can run in the app.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&decode_entities(&rest[..start]));
        let Some(end) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        let name: String =
            tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        let closing = tag.starts_with('/');
        rest = &rest[start + end + 1..];
        if !closing && (name == "script" || name == "style") {
            // Drop the contents too, not just the tags.
            let close = format!("</{name}");
            rest = rest.to_ascii_lowercase().find(&close).map_or("", |i| &rest[i..]);
            continue;
        }
        match name.as_str() {
            "br" => out.push('\n'),
            "p" | "div" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => out.push_str("\n\n"),
            "li" if !closing => out.push_str("\n• "),
            _ => {}
        }
    }
    out.push_str(&decode_entities(rest));

    // Tidy whitespace: trim each line, collapse runs of blank lines to one.
    let mut text = String::with_capacity(out.len());
    let mut blank_run = 0;
    for line in out.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")) {
        if line.is_empty() {
            blank_run += 1;
            continue;
        }
        if !text.is_empty() {
            text.push_str(if blank_run > 0 { "\n\n" } else { "\n" });
        }
        text.push_str(&line);
        blank_run = 0;
    }
    text
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&semi| semi <= 10).and_then(|semi| {
            let entity = &rest[1..semi];
            let ch = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                "mdash" => Some('—'),
                "ndash" => Some('–'),
                "hellip" => Some('…'),
                "rsquo" => Some('’'),
                "lsquo" => Some('‘'),
                "rdquo" => Some('”'),
                "ldquo" => Some('“'),
                "trade" => Some('™'),
                "reg" => Some('®'),
                "copy" => Some('©'),
                _ => entity
                    .strip_prefix("#x")
                    .or_else(|| entity.strip_prefix("#X"))
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                    .and_then(char::from_u32),
            };
            ch.map(|c| (c, semi))
        });
        match decoded {
            Some((c, semi)) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Formats Unix seconds as e.g. "9 Jul 2013" (UTC), matching how Steam writes dates.
pub(crate) fn format_unix_date(secs: i64) -> String {
    // Howard Hinnant's civil_from_days.
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{day} {} {year}", MONTHS[(month - 1) as usize])
}

/// Non-empty trimmed string field.
pub(crate) fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned)
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::*;

    /// Replays canned responses in order and records each request.
    #[derive(Default)]
    pub struct FakeHttp {
        pub responses: Mutex<VecDeque<Result<Value>>>,
        pub requests: Mutex<Vec<String>>,
    }

    impl FakeHttp {
        pub fn new(responses: impl IntoIterator<Item = Result<Value>>) -> Self {
            Self { responses: Mutex::new(responses.into_iter().collect()), ..Default::default() }
        }

        fn next(&self, request: String) -> Result<Value> {
            self.requests.lock().unwrap().push(request);
            self.responses.lock().unwrap().pop_front().expect("unexpected request")
        }
    }

    impl Http for FakeHttp {
        fn get_json(&self, url: &str, query: &[(&str, &str)]) -> Result<Value> {
            self.next(format!("GET {url} {query:?}"))
        }

        fn post_json(
            &self,
            url: &str,
            query: &[(&str, &str)],
            _: &[(&str, &str)],
            body: &str,
        ) -> Result<Value> {
            self.next(format!("POST {url} {query:?} {body}"))
        }
    }

    pub fn game(source: crate::Source, source_id: &str, title: &str) -> Game {
        Game {
            id: 1,
            source,
            source_id: source_id.into(),
            title: title.into(),
            install_dir: None,
            installed: false,
            size_bytes: None,
            last_updated: None,
            added_at: 0,
        }
    }

    struct Fixed(&'static str, Option<Result<Option<GameMetadata>>>);

    impl MetadataProvider for Fixed {
        fn id(&self) -> &'static str {
            self.0
        }
        fn supports(&self, _: &Game) -> bool {
            self.1.is_some()
        }
        fn fetch(&self, _: &dyn Http, _: &Game) -> Result<Option<GameMetadata>> {
            match self.1.as_ref().unwrap() {
                Ok(m) => Ok(m.clone()),
                Err(_) => Err(Error::Http { status: Some(503), message: "down".into() }),
            }
        }
    }

    fn named(provider: &str) -> GameMetadata {
        GameMetadata { provider: provider.into(), ..Default::default() }
    }

    #[test]
    fn fetch_takes_first_match_and_skips_unsupported() {
        let http = FakeHttp::default();
        let g = game(crate::Source::Steam, "1", "X");
        let unsupported = Fixed("a", None);
        let empty = Fixed("b", Some(Ok(None)));
        let hit = Fixed("c", Some(Ok(Some(named("c")))));
        let later = Fixed("d", Some(Ok(Some(named("d")))));
        let got = fetch(&[&unsupported, &empty, &hit, &later], &http, &g).unwrap();
        assert_eq!(got.unwrap().provider, "c");
    }

    #[test]
    fn fetch_reports_errors_only_when_nothing_matched() {
        let http = FakeHttp::default();
        let g = game(crate::Source::Steam, "1", "X");
        let down = Fixed("a", Some(Err(Error::Other(String::new()))));
        let empty = Fixed("b", Some(Ok(None)));
        let hit = Fixed("c", Some(Ok(Some(named("c")))));
        assert!(fetch(&[&down, &empty], &http, &g).is_err());
        assert_eq!(fetch(&[&down, &hit], &http, &g).unwrap().unwrap().provider, "c");
        assert_eq!(fetch(&[&empty], &http, &g).unwrap(), None);
    }

    #[test]
    fn html_to_text_keeps_structure_and_drops_markup() {
        let html = "<h2 class=\"bb_tag\">About</h2><p>Fight &amp; build.<br>New line&nbsp;here.</p>\
                    <ul class=\"bb_ul\"><li>One</li><li>Two &#8212; &#x2122;</li></ul>\
                    <img src=\"x.gif\"><script>alert(1)</script> AT&T";
        assert_eq!(html_to_text(html), "About\n\nFight & build.\nNew line here.\n\n• One\n• Two — ™\n\nAT&T");
    }

    #[test]
    fn formats_unix_dates() {
        assert_eq!(format_unix_date(0), "1 Jan 1970");
        assert_eq!(format_unix_date(1_373_328_000), "9 Jul 2013");
        assert_eq!(format_unix_date(951_782_400), "29 Feb 2000");
    }
}
