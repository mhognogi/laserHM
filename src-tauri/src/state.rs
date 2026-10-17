//! Starea aplicației, comună tuturor părților din Rust.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::grbl::Grbl;

/// Numele evenimentului prin care interfața află că s-a schimbat starea.
const STATUS_EVENT: &str = "laser-status";

/// Un laser conectat și setările citite de la el.
pub struct Laser {
    pub grbl: Grbl,
    /// Setarea GRBL `$30`: valoarea S care înseamnă 100% putere.
    pub max_power: f64,
}

#[derive(Default)]
pub struct AppState {
    pub laser: Mutex<Option<Arc<Laser>>>,
    /// true cât timp rulează o lucrare.
    pub busy: AtomicBool,
    message: Mutex<String>,
}

/// Ce trimitem interfeței (devine JSON).
#[derive(Clone, Serialize)]
pub struct Status {
    connected: bool,
    busy: bool,
    message: String,
}

impl AppState {
    pub fn laser(&self) -> Option<Arc<Laser>> {
        self.laser.lock().unwrap().clone()
    }

    pub fn status(&self) -> Status {
        Status {
            connected: self.laser().is_some(),
            busy: self.busy.load(Ordering::SeqCst),
            message: self.message.lock().unwrap().clone(),
        }
    }

    /// Schimbă mesajul de stare și anunță interfața.
    pub fn report(&self, app: &AppHandle, message: impl Into<String>) {
        *self.message.lock().unwrap() = message.into();
        let _ = app.emit(STATUS_EVENT, self.status());
    }
}
