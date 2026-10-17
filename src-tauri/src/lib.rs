mod commands;
mod connection;
mod grbl;
mod job;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::default())
        .setup(|app| {
            // Firul care caută laserul pornește odată cu aplicația.
            connection::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start,
            commands::stop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
