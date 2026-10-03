use std::sync::{Arc, Mutex};

use library_core::db::{unix_now, MergeStats};
use library_core::metadata::igdb::{Credentials, Igdb};
use library_core::metadata::steam::SteamStore;
use library_core::metadata::{self, CachedMetadata, MetadataProvider, UreqHttp};
use library_core::{steam, Game, Library, Source};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Shared app state: the open library database and the metadata providers.
struct AppState {
    library: Mutex<Library>,
    http: Arc<UreqHttp>,
    /// Set once the user has entered working IGDB credentials.
    igdb: Mutex<Option<Arc<Igdb>>>,
}

const IGDB_CLIENT_ID: &str = "igdb.client_id";
const IGDB_CLIENT_SECRET: &str = "igdb.client_secret";

/// Errors cross the IPC boundary as plain strings the UI can show.
type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
fn list_games(state: State<'_, AppState>) -> CmdResult<Vec<Game>> {
    state.library.lock().map_err(err)?.games().map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportReport {
    libraries: Vec<String>,
    found: usize,
    #[serde(flatten)]
    stats: MergeStats,
    warnings: Vec<String>,
}

/// Scans local Steam installs and merges what they report into the library.
#[tauri::command]
async fn import_steam(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ImportReport> {
    let home = app.path().home_dir().map_err(err)?;
    let scan = tauri::async_runtime::spawn_blocking(move || steam::scan(&home)).await.map_err(err)?;
    if scan.roots.is_empty() {
        return Err("Steam wasn't found. Looked in ~/.local/share/Steam, ~/.steam and the Flatpak and \
                    Snap locations."
            .into());
    }
    let stats = state.library.lock().map_err(err)?.merge_import(Source::Steam, &scan.games).map_err(err)?;
    Ok(ImportReport {
        libraries: scan.libraries.iter().map(|p| p.display().to_string()).collect(),
        found: scan.games.len(),
        stats,
        warnings: scan.warnings,
    })
}

/// Hands the game to its store client: play it if installed, otherwise install it.
#[tauri::command]
fn launch_game(app: AppHandle, state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    let game = state
        .library
        .lock()
        .map_err(err)?
        .games()
        .map_err(err)?
        .into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| format!("no game with id {id}"))?;
    let url = match game.source {
        Source::Steam if game.installed => format!("steam://rungameid/{}", game.source_id),
        Source::Steam => format!("steam://install/{}", game.source_id),
        other => return Err(format!("launching {} games isn't supported yet", other.as_str())),
    };
    app.opener().open_url(url, None::<&str>).map_err(err)
}

/// A game's metadata: the cached copy while it is fresh, otherwise looked up again.
/// `refresh` forces a lookup. If a background lookup fails, a stale cached copy is
/// returned rather than an error; `None` means the game was never found.
#[tauri::command]
async fn game_metadata(
    state: State<'_, AppState>,
    id: i64,
    refresh: bool,
) -> CmdResult<Option<CachedMetadata>> {
    let (game, cached) = {
        let library = state.library.lock().map_err(err)?;
        let game = library.game(id).map_err(err)?.ok_or_else(|| format!("no game with id {id}"))?;
        (game, library.cached_metadata(id).map_err(err)?)
    };
    if let Some(cached) = cached.as_ref().filter(|c| !refresh && c.is_fresh(unix_now())) {
        return Ok(Some(cached.clone()));
    }

    let http = state.http.clone();
    let igdb = state.igdb.lock().map_err(err)?.clone();
    let lookup = tauri::async_runtime::spawn_blocking(move || {
        let mut providers: Vec<&dyn MetadataProvider> = vec![&SteamStore];
        if let Some(igdb) = igdb.as_deref() {
            providers.push(igdb);
        }
        metadata::fetch(&providers, &*http, &game)
    })
    .await
    .map_err(err)?;

    match lookup {
        Ok(found) => {
            Ok(Some(state.library.lock().map_err(err)?.save_metadata(id, found.as_ref()).map_err(err)?))
        }
        Err(_) if !refresh && cached.is_some() => Ok(cached),
        Err(e) => Err(format!("Couldn't load game info: {e}")),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataSettings {
    /// The secret never goes back to the UI.
    igdb_client_id: Option<String>,
}

#[tauri::command]
fn metadata_settings(state: State<'_, AppState>) -> CmdResult<MetadataSettings> {
    let igdb = state.igdb.lock().map_err(err)?;
    Ok(MetadataSettings { igdb_client_id: igdb.as_ref().map(|p| p.credentials().client_id.clone()) })
}

/// Checks and saves the user's IGDB (Twitch) app credentials, or switches IGDB off
/// when both are empty. Games nothing had found before get looked up again.
#[tauri::command]
async fn set_igdb_credentials(
    state: State<'_, AppState>,
    client_id: String,
    client_secret: String,
) -> CmdResult<MetadataSettings> {
    let (client_id, client_secret) = (client_id.trim().to_owned(), client_secret.trim().to_owned());
    let provider = if client_id.is_empty() && client_secret.is_empty() {
        None
    } else if client_id.is_empty() || client_secret.is_empty() {
        return Err("Enter both the client ID and the client secret.".into());
    } else {
        let igdb = Arc::new(Igdb::new(Credentials { client_id, client_secret }));
        let (check, http) = (igdb.clone(), state.http.clone());
        tauri::async_runtime::spawn_blocking(move || check.check(&*http)).await.map_err(err)?.map_err(err)?;
        Some(igdb)
    };

    let library = state.library.lock().map_err(err)?;
    let creds = provider.as_ref().map(|p| p.credentials());
    library.set_setting(IGDB_CLIENT_ID, creds.map(|c| c.client_id.as_str())).map_err(err)?;
    library.set_setting(IGDB_CLIENT_SECRET, creds.map(|c| c.client_secret.as_str())).map_err(err)?;
    if provider.is_some() {
        library.forget_missing_metadata().map_err(err)?;
    }
    let igdb_client_id = creds.map(|c| c.client_id.clone());
    *state.igdb.lock().map_err(err)? = provider;
    Ok(MetadataSettings { igdb_client_id })
}

fn saved_igdb(library: &Library) -> library_core::Result<Option<Arc<Igdb>>> {
    let id = library.setting(IGDB_CLIENT_ID)?;
    let secret = library.setting(IGDB_CLIENT_SECRET)?;
    Ok(id
        .zip(secret)
        .map(|(client_id, client_secret)| Arc::new(Igdb::new(Credentials { client_id, client_secret }))))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_path = app.path().app_data_dir()?.join("library.db");
            let library = Library::open(&db_path)?;
            let igdb = saved_igdb(&library)?;
            app.manage(AppState {
                library: Mutex::new(library),
                http: Arc::new(UreqHttp::default()),
                igdb: Mutex::new(igdb),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_games,
            import_steam,
            launch_game,
            game_metadata,
            metadata_settings,
            set_igdb_credentials
        ])
        .run(tauri::generate_context!())
        .expect("error while running Game Library");
}
