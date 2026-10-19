//! User settings, saved to disk between runs
//! (settings.json in the app's config folder, e.g. %APPDATA%\com.mhogn.laserhm).

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::job::Point;
use crate::state::WorkArea;

/// Camera calibration: points clicked in the camera image and where they really are on
/// the bed, in the same order: the corners of a cut square, the middles of its sides and
/// its center (9; older calibrations have only the 4 corners). The UI turns them into
/// the perspectives that place the image on the bed.
#[derive(Clone, Serialize, Deserialize)]
pub struct CameraCalibration {
    /// In the camera image, as fractions of its width and height (0 to 1, from the top-left).
    pub image: Vec<Point>,
    /// On the bed, in mm (machine coordinates, Y up).
    pub bed: Vec<Point>,
}

/// The power, speed and passes last set in the laser panel.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobDefaults {
    pub power_percent: f64,
    /// mm/min
    pub feed: f64,
    pub passes: u32,
}

/// `default`: fields missing from an older settings.json get their default value
/// instead of making the whole file unreadable.
#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Work area set by the user; when present it is used even with a laser connected.
    pub work_area: Option<WorkArea>,
    /// The work area last read from a laser ($130/$131), used to pre-fill the dialog.
    pub laser_work_area: Option<WorkArea>,
    /// Ask for the pre-start checks (focus, head position, air assist) before cutting.
    pub check_before_start: bool,
    /// Places the camera image on the bed; `None` until the camera is calibrated.
    pub camera_calibration: Option<CameraCalibration>,
    /// The laser panel's last power, speed and passes; `None` until first changed.
    pub job: Option<JobDefaults>,
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("settings.json"))
}

/// Loads the settings; a missing or unreadable file gives the defaults.
pub fn load(app: &AppHandle) -> Settings {
    path(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app)?;
    let error = |e: std::io::Error| format!("Cannot save settings: {e}");
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(error)?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(error)
}
