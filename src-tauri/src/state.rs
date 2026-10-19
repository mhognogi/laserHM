//! Application state, shared by all Rust modules.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::grbl::Grbl;
use crate::settings::{CameraCalibration, JobDefaults, Settings};

/// Name of the event that tells the UI the state has changed.
const STATUS_EVENT: &str = "laser-status";

/// A connected laser and the settings read from it.
pub struct Laser {
    pub grbl: Grbl,
    /// GRBL setting `$30`: the S value that means 100% power.
    pub max_power: f64,
    pub work_area: WorkArea,
    /// GRBL setting `$110`: the speed of rapid (G0) moves, in mm/min.
    pub rapid_rate: f64,
}

/// The machine's work area in mm (GRBL settings `$130` and `$131`).
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct WorkArea {
    pub width: f64,
    pub height: f64,
}

/// What the laser is doing; in JSON "cut", "simulate", "home", "move" or "fire".
#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Task {
    Cut,
    Simulate,
    Home,
    /// Position Laser: the head moves to a point, laser off.
    Move,
    /// The laser is on without moving, while the Fire button is held.
    Fire,
}

#[derive(Default)]
pub struct AppState {
    pub laser: Mutex<Option<Arc<Laser>>>,
    /// The task running on the laser, or `None` when idle.
    task: Mutex<Option<Task>>,
    pub settings: Mutex<Settings>,
    message: Mutex<String>,
    /// True when `message` is an error (e.g. an ALARM from the laser).
    error: AtomicBool,
}

/// What we send to the UI (serialized as JSON, with camelCase field names).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    connected: bool,
    /// True while a task runs.
    busy: bool,
    /// The running task, or `None` (`null` in JSON).
    task: Option<Task>,
    /// True while the running job is paused.
    paused: bool,
    message: String,
    /// True when `message` is an error, so the UI never hides it.
    error: bool,
    /// The work area to display: the one set by the user, else the connected
    /// laser's. `None` if neither exists (becomes `null` in JSON).
    work_area: Option<WorkArea>,
    /// The work area last read from a laser, even if it is not connected now.
    laser_work_area: Option<WorkArea>,
    /// Ask for the pre-start checks before cutting (saved between sessions).
    check_before_start: bool,
    /// Places the camera image on the bed (saved between sessions); `null` until calibrated.
    camera_calibration: Option<CameraCalibration>,
    /// The laser panel's last power, speed and passes (saved between sessions).
    job: Option<JobDefaults>,
}

impl AppState {
    pub fn laser(&self) -> Option<Arc<Laser>> {
        self.laser.lock().unwrap().clone()
    }

    pub fn task(&self) -> Option<Task> {
        *self.task.lock().unwrap()
    }

    /// Marks `task` as running; fails if another task is already running.
    pub fn begin_task(&self, task: Task) -> Result<(), String> {
        let mut current = self.task.lock().unwrap();
        if current.is_some() {
            return Err("Laser is already running".into());
        }
        *current = Some(task);
        Ok(())
    }

    pub fn end_task(&self) {
        *self.task.lock().unwrap() = None;
    }

    pub fn status(&self) -> Status {
        let laser = self.laser();
        let task = self.task();
        let settings = self.settings.lock().unwrap();
        Status {
            connected: laser.is_some(),
            busy: task.is_some(),
            task,
            paused: laser.as_ref().is_some_and(|laser| laser.grbl.is_paused()),
            message: self.message.lock().unwrap().clone(),
            error: self.error.load(Ordering::SeqCst),
            work_area: settings.work_area.or(laser.map(|laser| laser.work_area)),
            laser_work_area: settings.laser_work_area,
            check_before_start: settings.check_before_start,
            camera_calibration: settings.camera_calibration.clone(),
            job: settings.job,
        }
    }

    /// Updates the status message and notifies the UI.
    pub fn report(&self, app: &AppHandle, message: impl Into<String>) {
        self.set_message(app, message.into(), false);
    }

    /// Like `report`, but marks the message as an error.
    pub fn report_error(&self, app: &AppHandle, message: impl Into<String>) {
        self.set_message(app, message.into(), true);
    }

    fn set_message(&self, app: &AppHandle, message: String, error: bool) {
        *self.message.lock().unwrap() = message;
        self.error.store(error, Ordering::SeqCst);
        self.notify(app);
    }

    /// Sends the current status to the UI.
    pub fn notify(&self, app: &AppHandle) {
        let _ = app.emit(STATUS_EVENT, self.status());
    }
}
