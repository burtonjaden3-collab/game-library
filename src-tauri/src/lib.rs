use std::sync::Mutex;

use library_core::db::MergeStats;
use library_core::{steam, Game, Library, Source};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Shared app state: the open library database.
struct AppState {
    library: Mutex<Library>,
}

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_path = app.path().app_data_dir()?.join("library.db");
            let library = Library::open(&db_path)?;
            app.manage(AppState { library: Mutex::new(library) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_games, import_steam, launch_game])
        .run(tauri::generate_context!())
        .expect("error while running Game Library");
}
