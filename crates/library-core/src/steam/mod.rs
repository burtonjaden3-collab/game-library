//! Steam importer: finds Steam installs, reads every library folder from
//! `libraryfolders.vdf`, and turns each `appmanifest_<appid>.acf` into a game.
//!
//! This reads local files only, so it sees installed (and installing) games.
//! Owned-but-not-installed games come from the Steam Web API, in [`web`].

pub mod vdf;
pub mod web;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::model::{ImportedGame, Source};

/// `StateFlags` bit Steam sets once an app is fully installed.
const STATE_FULLY_INSTALLED: u64 = 4;

/// Steamworks Common Redistributables: a tool, not a game, but not named like one.
const REDISTRIBUTABLES_APPID: &str = "228980";

/// Name prefixes of compatibility tools and runtimes Steam installs alongside games.
const TOOL_NAME_PREFIXES: &[&str] = &["Proton ", "Proton-", "Steam Linux Runtime", "Steamworks Common"];

/// Everything one Steam scan found.
#[derive(Debug, Default)]
pub struct SteamScan {
    /// Steam installs found (each holds its own `steamapps`).
    pub roots: Vec<PathBuf>,
    /// Library folders read across all roots.
    pub libraries: Vec<PathBuf>,
    pub games: Vec<ImportedGame>,
    /// Problems that skipped a file without stopping the scan.
    pub warnings: Vec<String>,
}

/// Where Steam keeps its data on this platform, most common first.
/// Only paths that exist are returned, and symlinked duplicates are dropped
/// (`~/.steam/steam` usually points at `~/.local/share/Steam`).
pub fn find_roots(home: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "linux") {
        candidates.extend([
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".steam/root"),
            home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
            home.join("snap/steam/common/.local/share/Steam"),
        ]);
    } else if cfg!(target_os = "macos") {
        candidates.push(home.join("Library/Application Support/Steam"));
    } else if cfg!(windows) {
        candidates.extend([
            PathBuf::from(r"C:\Program Files (x86)\Steam"),
            PathBuf::from(r"C:\Program Files\Steam"),
        ]);
    }
    dedup_existing(candidates.into_iter().filter(|p| p.join("steamapps").is_dir()))
}

/// Scans the Steam installs under `home`.
pub fn scan(home: &Path) -> SteamScan {
    scan_roots(&find_roots(home))
}

/// Scans the given Steam roots: every library folder they list, and every app
/// manifest in those folders. A game found in two places is reported once.
pub fn scan_roots(roots: &[PathBuf]) -> SteamScan {
    let mut scan = SteamScan { roots: roots.to_vec(), ..Default::default() };
    let mut libraries = Vec::new();
    for root in roots {
        libraries.push(root.clone());
        match library_folders(root) {
            Ok(found) => libraries.extend(found),
            Err(e) => scan.warnings.push(e),
        }
    }
    scan.libraries = dedup_existing(libraries.into_iter().filter(|p| p.join("steamapps").is_dir()));

    let mut seen = HashSet::new();
    for library in &scan.libraries {
        let steamapps = library.join("steamapps");
        let entries = match fs::read_dir(&steamapps) {
            Ok(entries) => entries,
            Err(e) => {
                scan.warnings.push(format!("{}: {e}", steamapps.display()));
                continue;
            }
        };
        let mut manifests: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"))
            })
            .collect();
        manifests.sort();
        for path in manifests {
            let parsed = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|text| parse_app_manifest(&text, library).map_err(|e| e.to_string()));
            match parsed {
                Ok(Some(game)) => {
                    if seen.insert(game.source_id.clone()) {
                        scan.games.push(game);
                    }
                }
                Ok(None) => {}
                Err(e) => scan.warnings.push(format!("{}: {e}", path.display())),
            }
        }
    }
    scan
}

/// The extra library folders a Steam root lists in `libraryfolders.vdf`.
/// Handles both the current format (`"0" { "path" "..." }`) and the pre-2021
/// one (`"1" "/path"`). A missing file means the root is the only library.
pub fn library_folders(root: &Path) -> Result<Vec<PathBuf>, String> {
    let candidates = [root.join("steamapps/libraryfolders.vdf"), root.join("config/libraryfolders.vdf")];
    let Some(path) = candidates.iter().find(|p| p.is_file()) else {
        return Ok(Vec::new());
    };
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc = vdf::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let Some(folders) = doc.get("libraryfolders") else {
        return Err(format!("{}: no \"libraryfolders\" block", path.display()));
    };
    Ok(folders
        .entries()
        .iter()
        // Library entries are keyed "0", "1", ...; other keys are metadata.
        .filter(|(key, _)| key.parse::<u32>().is_ok())
        .filter_map(|(_, value)| match value {
            vdf::Value::Obj(_) => value.get_str("path"),
            vdf::Value::Str(path) => Some(path.as_str()),
        })
        .map(PathBuf::from)
        .collect())
}

/// Turns one `appmanifest_<appid>.acf` into a game. Returns `Ok(None)` for
/// Proton, Steam runtimes and other tools that aren't games.
pub fn parse_app_manifest(text: &str, library: &Path) -> Result<Option<ImportedGame>, vdf::ParseError> {
    let doc = vdf::parse(text)?;
    let missing = |what: &str| vdf::ParseError { line: 0, message: format!("missing {what}") };
    let app = doc.get("AppState").ok_or_else(|| missing("\"AppState\" block"))?;
    let appid = app.get_str("appid").ok_or_else(|| missing("appid"))?.trim();
    if appid.is_empty() {
        return Err(missing("appid"));
    }
    let installdir = app.get_str("installdir").filter(|d| !d.is_empty());
    let name = app
        .get_str("name")
        .filter(|n| !n.is_empty())
        .or(installdir)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Steam app {appid}"));

    if appid == REDISTRIBUTABLES_APPID || TOOL_NAME_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return Ok(None);
    }

    let number = |key: &str| app.get_str(key).and_then(|v| v.trim().parse::<u64>().ok());
    let installed = number("StateFlags").is_some_and(|f| f & STATE_FULLY_INSTALLED != 0);
    Ok(Some(ImportedGame {
        source: Source::Steam,
        source_id: appid.to_owned(),
        title: name,
        install_dir: installdir
            .map(|d| library.join("steamapps/common").join(d).to_string_lossy().into_owned()),
        installed,
        size_bytes: number("SizeOnDisk").filter(|&s| s > 0),
        last_updated: number("LastUpdated").filter(|&t| t > 0).map(|t| t as i64),
        playtime_minutes: None,
        last_played: None,
    }))
}

/// Keeps paths that exist, dropping ones that resolve to an earlier entry.
fn dedup_existing(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths.into_iter().filter(|p| fs::canonicalize(p).map(|c| seen.insert(c)).unwrap_or(false)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(appid: &str, name: &str, installdir: &str, flags: u64) -> String {
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\t\"{appid}\"\n\t\"universe\"\t\t\"1\"\n\
             \t\"name\"\t\t\"{name}\"\n\t\"StateFlags\"\t\t\"{flags}\"\n\
             \t\"installdir\"\t\t\"{installdir}\"\n\t\"LastUpdated\"\t\t\"1727740800\"\n\
             \t\"SizeOnDisk\"\t\t\"123456789\"\n}}\n"
        )
    }

    fn write_manifest(library: &Path, appid: &str, name: &str, flags: u64) {
        let steamapps = library.join("steamapps");
        fs::create_dir_all(&steamapps).unwrap();
        fs::write(steamapps.join(format!("appmanifest_{appid}.acf")), manifest(appid, name, name, flags))
            .unwrap();
    }

    #[test]
    fn parses_a_real_manifest() {
        let game = parse_app_manifest(&manifest("570", "Dota 2", "dota 2 beta", 4), Path::new("/lib"))
            .unwrap()
            .unwrap();
        assert_eq!(
            game,
            ImportedGame {
                source: Source::Steam,
                source_id: "570".into(),
                title: "Dota 2".into(),
                install_dir: Some("/lib/steamapps/common/dota 2 beta".into()),
                installed: true,
                size_bytes: Some(123_456_789),
                last_updated: Some(1_727_740_800),
                playtime_minutes: None,
                last_played: None,
            }
        );
    }

    #[test]
    fn installed_follows_the_fully_installed_flag() {
        let lib = Path::new("/lib");
        let updating = parse_app_manifest(&manifest("1", "A", "A", 4 | 2 | 1024), lib).unwrap().unwrap();
        assert!(updating.installed, "a game with an update pending is still playable");
        let downloading = parse_app_manifest(&manifest("2", "B", "B", 1026), lib).unwrap().unwrap();
        assert!(!downloading.installed);
    }

    #[test]
    fn skips_proton_runtimes_and_redistributables() {
        let lib = Path::new("/lib");
        for (id, name) in [
            ("1493710", "Proton Experimental"),
            ("2805730", "Proton 9.0"),
            ("1628350", "Steam Linux Runtime 3.0 (sniper)"),
            ("228980", "Steamworks Common Redistributables"),
        ] {
            assert_eq!(parse_app_manifest(&manifest(id, name, name, 4), lib).unwrap(), None, "{name}");
        }
    }

    #[test]
    fn falls_back_to_installdir_for_the_title() {
        let text = "\"AppState\" { \"appid\" \"10\" \"installdir\" \"Half-Life\" \"StateFlags\" \"4\" }";
        let game = parse_app_manifest(text, Path::new("/lib")).unwrap().unwrap();
        assert_eq!(game.title, "Half-Life");
        assert_eq!(game.size_bytes, None);
    }

    #[test]
    fn rejects_manifests_without_an_appid() {
        assert!(parse_app_manifest("\"AppState\" { \"name\" \"x\" }", Path::new("/lib")).is_err());
        assert!(parse_app_manifest("\"Other\" { }", Path::new("/lib")).is_err());
    }

    #[test]
    fn scans_every_library_folder_in_both_vdf_formats() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Steam");
        let second = tmp.path().join("SecondDrive/SteamLibrary");
        let third = tmp.path().join("ThirdDrive/SteamLibrary");
        write_manifest(&root, "570", "Dota 2", 4);
        write_manifest(&root, "1493710", "Proton Experimental", 4);
        write_manifest(&second, "620", "Portal 2", 4);
        write_manifest(&third, "400", "Portal", 1026);
        fs::write(root.join("steamapps/appmanifest_bad.acf"), "\"AppState\" {").unwrap();

        // Current format: the root lists itself plus the second drive, with
        // a folder that no longer exists.
        fs::write(
            root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\" {{ \"570\" \"1\" }}\n\t}}\n\
                 \t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n\t\"2\" {{ \"path\" \"/does/not/exist\" }}\n}}\n",
                root.display(),
                second.display()
            ),
        )
        .unwrap();
        // Old format, in a second Steam root.
        let old_root = tmp.path().join("OldSteam");
        fs::create_dir_all(old_root.join("steamapps")).unwrap();
        fs::write(
            old_root.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"LibraryFolders\"\n{{\n\t\"TimeNextStatsReport\"\t\"1\"\n\t\"ContentStatsID\"\t\"-1\"\n\
                 \t\"1\"\t\"{}\"\n\t\"2\"\t\"{}\"\n}}\n",
                third.display(),
                second.display()
            ),
        )
        .unwrap();

        let scan = scan_roots(&[root.clone(), old_root.clone()]);
        assert_eq!(scan.libraries, [root, second, old_root, third]);
        let mut found: Vec<_> = scan.games.iter().map(|g| (g.source_id.as_str(), g.installed)).collect();
        found.sort();
        assert_eq!(found, [("400", false), ("570", true), ("620", true)]);
        assert_eq!(scan.warnings.len(), 1, "{:?}", scan.warnings);
        assert!(scan.warnings[0].contains("appmanifest_bad.acf"));
    }

    #[cfg(unix)]
    #[test]
    fn finds_linux_roots_and_drops_symlinked_duplicates() {
        let home = tempfile::tempdir().unwrap();
        let native = home.path().join(".local/share/Steam");
        fs::create_dir_all(native.join("steamapps")).unwrap();
        fs::create_dir_all(home.path().join(".steam")).unwrap();
        std::os::unix::fs::symlink(&native, home.path().join(".steam/steam")).unwrap();
        let flatpak = home.path().join(".var/app/com.valvesoftware.Steam/.local/share/Steam");
        fs::create_dir_all(flatpak.join("steamapps")).unwrap();

        if cfg!(target_os = "linux") {
            assert_eq!(find_roots(home.path()), [native, flatpak]);
        }
    }

    #[test]
    fn no_steam_means_an_empty_scan() {
        let home = tempfile::tempdir().unwrap();
        let scan = scan(home.path());
        assert!(scan.roots.is_empty() && scan.games.is_empty() && scan.warnings.is_empty());
    }
}
