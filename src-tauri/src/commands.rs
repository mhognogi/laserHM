//! Functions the UI can call with `invoke(...)`.

use std::cell::RefCell;
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::boolean;
use crate::job::{self, JobSettings, Shape};
use crate::material_test::{self, MaterialTest};
use crate::settings::{self, CameraCalibration, JobDefaults};
use crate::text;
use crate::state::{AppState, Laser, Status, Task, WorkArea};

/// Event announcing what the laser is cutting with (see `run_task`).
const CUT_PROGRESS_EVENT: &str = "cut-progress";
/// Event announcing how far the running job is (see `run_task`).
const JOB_PROGRESS_EVENT: &str = "job-progress";

#[tauri::command]
pub fn get_status(state: State<AppState>) -> Status {
    state.status()
}

/// Saves the work area set by the user; it takes precedence over the laser's.
#[tauri::command]
pub fn set_work_area(
    app: AppHandle,
    state: State<AppState>,
    width: f64,
    height: f64,
) -> Result<(), String> {
    if !(width > 0.0 && height > 0.0) {
        return Err("Width and height must be greater than 0".into());
    }
    // The block limits how long `settings` stays locked: the lock is released
    // at the closing brace, before `notify` reads the settings again.
    {
        let mut settings = state.settings.lock().unwrap();
        settings.work_area = Some(WorkArea { width, height });
        settings::save(&app, &settings)?;
    }
    state.notify(&app);
    Ok(())
}

/// Saves whether Start asks for the pre-start checks first.
#[tauri::command]
pub fn set_check_before_start(app: AppHandle, state: State<AppState>, enabled: bool) -> Result<(), String> {
    {
        let mut settings = state.settings.lock().unwrap();
        settings.check_before_start = enabled;
        settings::save(&app, &settings)?;
    }
    state.notify(&app);
    Ok(())
}

/// Saves the laser panel's power, speed and passes, so the next run starts with them.
/// Not announced to the UI: it already shows them.
#[tauri::command]
pub fn set_job_settings(app: AppHandle, state: State<AppState>, job: JobDefaults) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.job = Some(job);
    settings::save(&app, &settings)
}

/// Saves the camera calibration: the points clicked in the image and the bed points they
/// show; 9 (corners, middles of the sides, center) or, from older versions, 4 (corners).
#[tauri::command]
pub fn set_camera_calibration(
    app: AppHandle,
    state: State<AppState>,
    calibration: CameraCalibration,
) -> Result<(), String> {
    let count = calibration.image.len();
    if !(count == 9 || count == 4) || calibration.bed.len() != count {
        return Err("Camera calibration needs 9 points".into());
    }
    {
        let mut settings = state.settings.lock().unwrap();
        settings.camera_calibration = Some(calibration);
        settings::save(&app, &settings)?;
    }
    state.notify(&app);
    Ok(())
}

/// Cuts the selected shapes at their positions on the work area (absolute machine
/// coordinates), with the power (%), speed (mm/min) and passes chosen in the UI.
/// From JavaScript: invoke("start", { powerPercent, feed, passes, shapes: [{ type, cx, cy, rx, ry }, …] }).
#[tauri::command]
pub fn start(
    app: AppHandle,
    state: State<AppState>,
    power_percent: f64,
    feed: f64,
    passes: u32,
    shapes: Vec<Shape>,
) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    let settings = JobSettings::new(power_percent, feed, passes)?;
    // Safety: never send the head beyond the machine's travel ($130/$131).
    job::check_shapes(&shapes, laser.work_area)?;
    job::check_rasters(&shapes)?;
    let (gcode, changes) = job::cut_job(&shapes, &settings, laser.max_power, laser.work_area);
    // `from_home` adds one line ($H) before the job.
    let changes = changes.into_iter().map(|(index, progress)| (index + 1, progress)).collect();
    run_task(app, &state, laser, Task::Cut, from_home(gcode), changes, "Cutting…", "Done. Connected to laser")
}

/// Follows the selected shapes once with the laser off, at the chosen speed.
#[tauri::command]
pub fn simulate(app: AppHandle, state: State<AppState>, feed: f64, shapes: Vec<Shape>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    // Same checks as a real cut; power and passes do not matter here.
    JobSettings::new(0.0, feed, 1)?;
    job::check_shapes(&shapes, laser.work_area)?;
    let gcode = from_home(job::simulation_gcode(&shapes, feed));
    run_task(app, &state, laser, Task::Simulate, gcode, Vec::new(), "Simulating (laser off)…", "Simulation done")
}

/// The Material Test's cells and labels at their place on the bed, for the dialog's preview.
#[tauri::command]
pub async fn material_test_preview(test: MaterialTest) -> Result<Vec<material_test::Part>, String> {
    // Background thread: the labels read the font file.
    tauri::async_runtime::spawn_blocking(move || test.parts()).await.map_err(|e| e.to_string())?
}

/// Start and Sim always begin with homing (`$H`, as the Home button), so the job
/// starts from the machine's origin; GRBL answers it only once the head is there.
fn from_home(job: Vec<String>) -> Vec<String> {
    std::iter::once("$H".to_string()).chain(job).collect()
}

/// The fonts installed in Windows, sorted by name.
/// `async` + `spawn_blocking`: the first call reads every font file, which takes
/// a moment, so it runs on a background thread instead of freezing the window.
#[tauri::command]
pub async fn list_fonts() -> Vec<text::FontInfo> {
    tauri::async_runtime::spawn_blocking(text::installed_fonts).await.unwrap_or_default()
}

/// The outlines of `text` in a font (see text.rs). `welded` may be missing
/// (texts made before the option existed): then the letters are not joined.
#[tauri::command]
pub async fn text_outline(
    text: String,
    path: String,
    index: u32,
    welded: Option<bool>,
) -> Result<text::TextOutline, String> {
    let welded = welded.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || text::outline(&text, &path, index, welded))
        .await
        .map_err(|e| e.to_string())?
}

/// Weld, Union, Subtract or Intersection of the selected shapes (see boolean.rs):
/// the outlines of the result, in machine coordinates.
#[tauri::command]
pub fn boolean(op: boolean::BooleanOp, shapes: Vec<Shape>) -> Result<Vec<Vec<job::Point>>, String> {
    boolean::combine(op, &shapes)
}

/// Homing (`$H`): the head moves to the machine's origin using the limit switches,
/// at the laser's own homing speed (`$25`).
#[tauri::command]
pub fn home(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    let gcode = vec!["$H".to_string()];
    run_task(app, &state, laser, Task::Home, gcode, Vec::new(), "Homing…", "At origin. Connected to laser")
}

/// Position Laser: moves the head to (x, y) mm, with the laser off, at the machine's
/// fastest travel speed (G0, limited by `$110`/`$111`).
#[tauri::command]
pub fn move_to(app: AppHandle, state: State<AppState>, x: f64, y: f64) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    let gcode = job::move_gcode(x, y, laser.work_area)?;
    run_task(app, &state, laser, Task::Move, gcode, Vec::new(), "Moving the laser head…", "In position. Connected to laser")
}

/// Fire: turns the laser on at `power_percent` % of `$30`, without moving, until
/// `fire_off` (the UI calls it when the Fire button is released) or Stop.
#[tauri::command]
pub fn fire(app: AppHandle, state: State<AppState>, power_percent: f64) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    if !(0.0..=100.0).contains(&power_percent) {
        return Err("Fire power must be between 0 and 100%".into());
    }
    state.begin_task(Task::Fire)?;
    let power = (laser.max_power * power_percent / 100.0).round();
    if let Err(error) = laser.grbl.run(&job::fire_gcode(power), laser.grbl.stop_generation()) {
        // Whatever was sent, make sure the laser is off.
        let _ = laser.grbl.command("M5 S0");
        state.end_task();
        state.report_error(&app, error.clone());
        return Err(error);
    }
    state.report(&app, "Laser on (Fire)");
    Ok(())
}

/// Turns off the laser turned on by `fire`. Does nothing when it is not on.
#[tauri::command]
pub fn fire_off(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    if state.task() != Some(Task::Fire) {
        return Ok(());
    }
    let result = match state.laser() {
        Some(laser) => laser.grbl.command("M5 S0").map(|_| ()),
        None => Ok(()),
    };
    state.end_task();
    match result {
        Ok(()) => state.report(&app, "Laser off. Connected to laser"),
        Err(error) => state.report_error(&app, error),
    }
    Ok(())
}

/// Runs `gcode` on a separate thread (so the UI does not freeze) as `task`,
/// reporting `running` meanwhile and `done` (or the error) at the end.
/// `changes`: where the cut settings change. While it runs, `Tracker` follows the head's
/// position and announces the cut settings and how far the job is.
#[allow(clippy::too_many_arguments)]
fn run_task(
    app: AppHandle,
    state: &AppState,
    laser: Arc<Laser>,
    task: Task,
    gcode: Vec<String>,
    changes: Vec<(usize, job::CutProgress)>,
    running: &'static str,
    done: &'static str,
) -> Result<(), String> {
    state.begin_task(task)?;
    // Capture the Stop counter now: if Stop is pressed before the task
    // actually starts, it does not start at all.
    let generation = laser.grbl.stop_generation();
    state.report(&app, running);

    // After each line: its end time and where the head then is, to tell how far the job is.
    let trace = job::trace(&gcode, laser.rapid_rate);
    let shows_progress = matches!(task, Task::Cut | Task::Simulate);

    thread::spawn(move || {
        let tracker = RefCell::new(Tracker::new(&app, trace, changes, shows_progress));
        let result = laser
            .grbl
            .run_with(
                &gcode,
                generation,
                // The first move (after homing, $H) starts the job's time.
                |index| {
                    if !gcode[index].starts_with('$') {
                        tracker.borrow_mut().start(index);
                    }
                },
                |accepted, position| tracker.borrow_mut().head_at(accepted, position),
            )
            .map(|_| ());
        let _ = app.emit(CUT_PROGRESS_EVENT, None::<job::CutProgress>);
        // Done: the bar stays full until the next job; after a Stop or an error it stays where it was.
        if result.is_ok() {
            tracker.borrow_mut().finish();
        }
        let state = app.state::<AppState>();
        state.end_task();
        match result {
            Ok(()) => state.report(&app, done),
            Err(error) => state.report_error(&app, error),
        }
    });
    Ok(())
}

/// Follows a running job by where the head is, and announces it to the UI: how far the
/// job is ("job-progress", at each whole percent) and the cut settings of the line being
/// cut ("cut-progress").
struct Tracker<'a> {
    app: &'a AppHandle,
    /// After each line: its end time (s) and the head's position.
    trace: Vec<(f64, (f64, f64))>,
    total_seconds: f64,
    /// Where the cut settings change, the next first.
    changes: std::iter::Peekable<std::vec::IntoIter<(usize, job::CutProgress)>>,
    shows_progress: bool,
    /// The line the head is on; None before the first move.
    current: Option<usize>,
    percent: f64,
}

impl<'a> Tracker<'a> {
    fn new(
        app: &'a AppHandle,
        trace: Vec<(f64, (f64, f64))>,
        changes: Vec<(usize, job::CutProgress)>,
        shows_progress: bool,
    ) -> Tracker<'a> {
        let total_seconds = trace.last().map(|line| line.0).unwrap_or(0.0);
        Tracker {
            app,
            trace,
            total_seconds,
            changes: changes.into_iter().peekable(),
            shows_progress,
            current: None,
            percent: -1.0,
        }
    }

    /// The first move is being sent: the job starts at 0 %.
    fn start(&mut self, index: usize) {
        if self.current.is_none() {
            self.current = Some(index);
            self.announce(index, 0.0);
        }
    }

    /// The head is at `position`, and GRBL has accepted the lines before `accepted`:
    /// it is on one of the lines from the current one up to there, the nearest to it.
    fn head_at(&mut self, accepted: usize, position: (f64, f64)) {
        let Some(current) = self.current else { return };
        let last = accepted.clamp(current + 1, self.trace.len()) - 1;
        let mut best = (current, f64::INFINITY, 0.0);
        for line in current..=last {
            let from = if line == 0 { self.trace[0].1 } else { self.trace[line - 1].1 };
            let (distance, along) = distance_to_segment(position, from, self.trace[line].1);
            if distance < best.1 {
                best = (line, distance, along);
            }
        }
        let (line, _, along) = best;
        self.current = Some(line);
        // The time when the line starts, plus the part of it already done.
        let start = if line == 0 { 0.0 } else { self.trace[line - 1].0 };
        let seconds = start + (self.trace[line].0 - start) * along;
        self.announce(line, seconds);
    }

    fn finish(&mut self) {
        if self.shows_progress {
            let progress = job::JobProgress { fraction: 1.0, total_seconds: self.total_seconds };
            let _ = self.app.emit(JOB_PROGRESS_EVENT, progress);
        }
    }

    /// Announces the cut settings reached by `line`, and the percent when it changes.
    fn announce(&mut self, line: usize, seconds: f64) {
        while let Some((_, progress)) = self.changes.next_if(|(at, _)| *at <= line) {
            let _ = self.app.emit(CUT_PROGRESS_EVENT, Some(progress));
        }
        if !self.shows_progress {
            return;
        }
        let fraction = if self.total_seconds > 0.0 { (seconds / self.total_seconds).min(1.0) } else { 0.0 };
        if (fraction * 100.0).floor() != self.percent {
            self.percent = (fraction * 100.0).floor();
            let progress = job::JobProgress { fraction, total_seconds: self.total_seconds };
            let _ = self.app.emit(JOB_PROGRESS_EVENT, progress);
        }
    }
}

/// How far `point` is from the segment `a`–`b`, and where along it (0 at `a`, 1 at `b`)
/// its nearest point is.
fn distance_to_segment(point: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    let along = if length_squared > 0.0 {
        (((point.0 - a.0) * dx + (point.1 - a.1) * dy) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let nearest = (a.0 + dx * along, a.1 + dy * along);
    ((point.0 - nearest.0).hypot(point.1 - nearest.1), along)
}

/// Pauses the running cut or simulation; `resume` continues it.
#[tauri::command]
pub fn pause(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    match state.task() {
        Some(Task::Cut | Task::Simulate) => {}
        Some(Task::Home) => return Err("Homing cannot be paused".into()),
        Some(Task::Move) => return Err("Moving the head cannot be paused".into()),
        Some(Task::Fire) => return Err("Fire cannot be paused".into()),
        None => return Err("No job is running".into()),
    }
    laser.grbl.pause()?;
    state.report(&app, "Paused. Press Start to resume");
    Ok(())
}

#[tauri::command]
pub fn resume(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    laser.grbl.resume()?;
    let message = if state.task() == Some(Task::Simulate) { "Simulating (laser off)…" } else { "Cutting…" };
    state.report(&app, message);
    Ok(())
}

#[tauri::command]
pub fn stop(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let laser = state.laser().ok_or("Laser is not connected")?;
    thread::spawn(move || {
        let state = app.state::<AppState>();
        let result = laser.grbl.stop();
        // Fire has no thread of its own to end its task: the reset turned the laser off.
        if state.task() == Some(Task::Fire) {
            state.end_task();
        }
        match result {
            Ok(()) => state.report(&app, "Stopped. Connected to laser"),
            Err(error) => state.report_error(&app, error),
        }
    });
    Ok(())
}
