//! Projects (.lhm files, File > Open / Save) and the last session, saved automatically so
//! the app starts where it was left. Both hold the same JSON text, made by the UI
//! (see project.ts); Rust only writes and reads it.

use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// The last session, next to settings.json in the app's config folder.
fn session_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("session.json"))
}

#[tauri::command]
pub fn save_project(path: String, content: String) -> Result<(), String> {
    fs::write(&path, content).map_err(|e| format!("Cannot save {path}: {e}"))
}

#[tauri::command]
pub fn open_project(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| format!("Cannot open {path}: {e}"))
}

#[tauri::command]
pub fn save_session(app: AppHandle, content: String) -> Result<(), String> {
    let path = session_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(&path, content).map_err(|e| format!("Cannot save the session: {e}"))
}

/// The last session, or `None` on the first run (or if the file cannot be read).
#[tauri::command]
pub fn load_session(app: AppHandle) -> Option<String> {
    fs::read_to_string(session_path(&app).ok()?).ok()
}
