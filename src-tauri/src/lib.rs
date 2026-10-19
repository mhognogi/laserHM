mod boolean;
mod commands;
mod connection;
mod grbl;
mod job;
mod material_test;
pub mod mcp;
mod project;
mod settings;
mod state;
mod text;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // The Windows Open / Save dialogs (File menu).
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::default())
        .manage(mcp::McpState::default())
        .setup(|app| {
            // Load the settings saved in a previous run.
            let state = app.state::<state::AppState>();
            *state.settings.lock().unwrap() = settings::load(app.handle());

            // The thread that searches for the laser starts with the app.
            connection::spawn(app.handle().clone());
            // Claude Desktop can draw on the canvas (see mcp.rs).
            mcp::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::set_work_area,
            commands::set_check_before_start,
            commands::set_job_settings,
            commands::set_camera_calibration,
            commands::start,
            commands::simulate,
            commands::material_test_preview,
            commands::home,
            commands::move_to,
            commands::fire,
            commands::fire_off,
            commands::boolean,
            commands::list_fonts,
            commands::text_outline,
            commands::pause,
            commands::resume,
            commands::stop,
            mcp::mcp_respond,
            project::save_project,
            project::open_project,
            project::save_session,
            project::load_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
