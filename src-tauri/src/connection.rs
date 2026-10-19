//! Finds the laser on the USB ports and reconnects if the link is lost.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serialport::SerialPortType;
use tauri::{AppHandle, Manager};

use crate::grbl::Grbl;
use crate::settings;
use crate::state::{AppState, Laser, WorkArea};

const RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// Starts a thread that keeps the laser connected while the app is running.
pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let state = app.state::<AppState>();
        state.report(&app, "Searching for laser…");

        loop {
            match state.laser() {
                Some(laser) if laser.grbl.is_alive() => {}
                previous => {
                    if previous.is_some() {
                        *state.laser.lock().unwrap() = None;
                        state.report(&app, "Laser disconnected. Searching again…");
                    }
                    if let Some(laser) = find_laser() {
                        remember_laser_work_area(&app, &state, laser.work_area);
                        *state.laser.lock().unwrap() = Some(Arc::new(laser));
                        state.report(&app, "Connected to laser");
                    }
                }
            }
            thread::sleep(RETRY_INTERVAL);
        }
    });
}

/// Saves the laser's work area so it is known even when the laser is off.
fn remember_laser_work_area(app: &AppHandle, state: &AppState, work_area: WorkArea) {
    let mut settings = state.settings.lock().unwrap();
    settings.laser_work_area = Some(work_area);
    // Not fatal: the laser is connected and usable even if saving fails.
    if let Err(error) = settings::save(app, &settings) {
        eprintln!("{error}");
    }
}

/// Tries the USB ports one by one and returns the first one that is a GRBL laser.
fn find_laser() -> Option<Laser> {
    let ports = serialport::available_ports().unwrap_or_default();
    // Diagnostics: `{:?}` prints the value in its debug form (Debug).
    eprintln!("Ports found: {:?}", ports);
    ports
        .iter()
        .filter(|port| matches!(port.port_type, SerialPortType::UsbPort(_)))
        .find_map(|port| match connect(&port.port_name) {
            Ok(laser) => Some(laser),
            Err(error) => {
                eprintln!("[{}] connection failed: {error}", port.port_name);
                None
            }
        })
}

fn connect(port_name: &str) -> Result<Laser, String> {
    let grbl = Grbl::open(port_name)?;
    let settings = grbl.settings()?;
    // Closure: a small unnamed function that "sees" the `settings` variable.
    let setting = |number: u32| {
        settings
            .get(&number)
            .copied()
            .ok_or_else(|| format!("Laser did not report setting ${number}"))
    };
    Ok(Laser {
        max_power: setting(30)?,
        work_area: WorkArea { width: setting(130)?, height: setting(131)? },
        // Only for the time estimate: a common value when the laser does not say.
        rapid_rate: setting(110).unwrap_or(3000.0),
        grbl,
    })
}
