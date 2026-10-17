//! Funcțiile pe care interfața le poate apela cu `invoke(...)`.

use std::sync::atomic::Ordering;
use std::thread;

use tauri::{AppHandle, Manager, State};

use crate::job;
use crate::state::{AppState, Status};

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Status {
    state.status()
}

#[tauri::command]
pub fn start(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laserul nu este conectat")?;
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err("Laserul lucrează deja".into());
    }
    // Reținem contorul de Stop acum: dacă se apasă Stop înainte ca lucrarea
    // să înceapă efectiv, ea nu mai pornește deloc.
    let generation = laser.grbl.stop_generation();
    state.report(&app, "Taie linia…");

    // Lucrarea rulează pe un fir separat ca interfața să nu înghețe.
    thread::spawn(move || {
        let result = laser.grbl.run(&job::EXAMPLE_LINE.gcode(laser.max_power), generation);
        let state = app.state::<AppState>();
        state.busy.store(false, Ordering::SeqCst);
        match result {
            Ok(_) => state.report(&app, format!("Gata. Conectat pe {}", laser.grbl.port_name())),
            Err(error) => state.report(&app, error),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn stop(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laserul nu este conectat")?;
    thread::spawn(move || {
        let state = app.state::<AppState>();
        match laser.grbl.stop() {
            Ok(()) => state.report(&app, format!("Oprit. Conectat pe {}", laser.grbl.port_name())),
            Err(error) => state.report(&app, error),
        }
    });
    Ok(())
}
