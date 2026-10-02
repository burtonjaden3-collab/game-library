use std::sync::Mutex;

use library_core::{Game, Library};
use tauri::{Manager, State};

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
        .invoke_handler(tauri::generate_handler![list_games])
        .run(tauri::generate_context!())
        .expect("error while running Game Library");
}
