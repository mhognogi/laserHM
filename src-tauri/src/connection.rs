//! Găsește laserul pe porturile USB și se reconectează dacă legătura se pierde.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serialport::SerialPortType;
use tauri::{AppHandle, Manager};

use crate::grbl::Grbl;
use crate::state::{AppState, Laser};

const RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// Pornește un fir de execuție care ține legătura cu laserul cât timp rulează aplicația.
pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        let state = app.state::<AppState>();
        state.report(&app, "Caut laserul…");

        loop {
            match state.laser() {
                Some(laser) if laser.grbl.is_alive() => {}
                previous => {
                    if previous.is_some() {
                        *state.laser.lock().unwrap() = None;
                        state.report(&app, "Laserul s-a deconectat. Caut din nou…");
                    }
                    if let Some(laser) = find_laser() {
                        let message = format!("Conectat pe {}", laser.grbl.port_name());
                        *state.laser.lock().unwrap() = Some(Arc::new(laser));
                        state.report(&app, message);
                    }
                }
            }
            thread::sleep(RETRY_INTERVAL);
        }
    });
}

/// Încearcă pe rând porturile USB și îl întoarce pe primul care e un laser GRBL.
fn find_laser() -> Option<Laser> {
    let ports = serialport::available_ports().unwrap_or_default();
    // Diagnostic: `{:?}` afișează valoarea în forma ei de depanare (Debug).
    eprintln!("Porturi găsite: {:?}", ports);
    ports
        .iter()
        .filter(|port| matches!(port.port_type, SerialPortType::UsbPort(_)))
        .find_map(|port| match connect(&port.port_name) {
            Ok(laser) => Some(laser),
            Err(error) => {
                eprintln!("[{}] conectare eșuată: {error}", port.port_name);
                None
            }
        })
}

fn connect(port_name: &str) -> Result<Laser, String> {
    let grbl = Grbl::open(port_name)?;
    let max_power = grbl.setting(30)?;
    Ok(Laser { grbl, max_power })
}
