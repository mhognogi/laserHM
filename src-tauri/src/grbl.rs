//! Communication with the GRBL controller over the serial (USB) port.
//!
//! Streams G-code with "character counting" (see `Grbl::run_with`): lines are sent
//! while they fit in GRBL's receive buffer; each "ok" (or "error:N") answers the oldest.

use std::collections::{HashMap, VecDeque};
use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serialport::SerialPort;

const BAUD_RATE: u32 = 115_200;
/// Ctrl-X: GRBL "soft reset". Immediately stops motion and the laser.
const SOFT_RESET: u8 = 0x18;
/// GRBL "feed hold": decelerates to a stop and holds; the job can be resumed.
const FEED_HOLD: u8 = b'!';
/// GRBL "cycle start": resumes after a feed hold.
const CYCLE_START: u8 = b'~';
/// How long to wait for the "Grbl ..." startup message after opening the port.
const BANNER_TIMEOUT: Duration = Duration::from_millis(2500);
/// How long to wait for the startup message after a soft reset.
const RESET_TIMEOUT: Duration = Duration::from_millis(1500);
/// How long the laser must stay silent after a reset to be considered ready.
const QUIET_TIME: Duration = Duration::from_millis(300);
/// GRBL realtime command: report the status (state and position) right away.
const STATUS_QUERY: u8 = b'?';
/// How often the position is asked for while a job runs.
const STATUS_INTERVAL: Duration = Duration::from_millis(200);
/// GRBL's serial receive buffer is 128 bytes, but a ring buffer keeps one free: it holds
/// at most 127 characters. One more is lost, and with it maybe a line's end (see `run_with`).
const RX_BUFFER_CHARS: usize = 127;
/// How long to wait for "ok" to a command before giving up.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// How often to check for a stop request while waiting for a response.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct Grbl {
    port_name: String,
    /// The writing half of the port. The mutex lets the Stop button
    /// write even while a job is waiting for a response.
    writer: Mutex<Box<dyn SerialPort>>,
    /// Lines received from the laser, read by a separate thread.
    lines: Mutex<Receiver<String>>,
    /// Becomes false when the port stops responding (e.g. cable unplugged).
    alive: Arc<AtomicBool>,
    /// Incremented on every Stop. A job started before the Stop sees
    /// that the number changed and aborts.
    stop_generation: AtomicU64,
    /// True between Pause (feed hold) and Resume or Stop.
    paused: AtomicBool,
}

impl Grbl {
    /// Opens the port and checks that a GRBL controller is on the other end.
    pub fn open(port_name: &str) -> Result<Grbl, String> {
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(POLL_INTERVAL)
            .open()
            .map_err(|e| format!("Cannot open {port_name}: {e}"))?;
        let reader = port
            .try_clone()
            .map_err(|e| format!("Cannot use {port_name}: {e}"))?;

        let (tx, rx) = mpsc::channel();
        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();
        let reader_name = port_name.to_string();
        thread::spawn(move || read_lines(reader, reader_name, tx, reader_alive));

        let grbl = Grbl {
            port_name: port_name.to_string(),
            writer: Mutex::new(port),
            lines: Mutex::new(rx),
            alive,
            stop_generation: AtomicU64::new(0),
            paused: AtomicBool::new(false),
        };

        // Many boards reset when the port opens and send the startup
        // message on their own. If not, we ask for a reset ourselves.
        let lines = grbl.lines.lock().unwrap();
        let found = wait_for_banner(&lines, BANNER_TIMEOUT) || {
            grbl.write(&[SOFT_RESET])?;
            wait_for_banner(&lines, RESET_TIMEOUT)
        };
        drop(lines);

        if found {
            Ok(grbl)
        } else {
            Err(format!("{port_name} does not respond like a GRBL controller"))
        }
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Current value of the Stop counter; passed to `run`.
    pub fn stop_generation(&self) -> u64 {
        self.stop_generation.load(Ordering::SeqCst)
    }

    /// Sends a single command and returns the lines received before "ok".
    pub fn command(&self, command: &str) -> Result<Vec<String>, String> {
        self.run(&[command.to_string()], self.stop_generation())
    }

    /// Reads all GRBL settings (`$$`): setting number → value.
    /// E.g. the line "$30=1000.000" becomes the pair 30 → 1000.0.
    pub fn settings(&self) -> Result<HashMap<u32, f64>, String> {
        Ok(self
            .command("$$")?
            .iter()
            .filter_map(|line| {
                let (number, value) = line.strip_prefix('$')?.split_once('=')?;
                Some((number.parse().ok()?, value.trim().parse().ok()?))
            })
            .collect())
    }

    /// Sends the commands and waits until all are answered with "ok".
    /// Aborts if Stop was pressed meanwhile (`generation` is no longer current).
    pub fn run(&self, commands: &[String], generation: u64) -> Result<Vec<String>, String> {
        self.run_with(commands, generation, |_| {}, |_, _| {})
    }

    /// Like `run`, calling `on_send` with each command's index just before sending it,
    /// and `on_position` with the head's position (work coordinates) and how many
    /// commands GRBL has accepted so far, a few times a second while it works.
    ///
    /// "Character counting", as LightBurn streams: commands are sent while they fit in
    /// GRBL's receive buffer, without waiting for each "ok", so GRBL always has the next
    /// moves ready (an image's thousands of short moves would otherwise stutter).
    /// Each "ok" answers the oldest command still waiting and frees its characters.
    /// System commands (`$H`, `$X`, `$$`…) are sent alone, after everything before them.
    pub fn run_with(
        &self,
        commands: &[String],
        generation: u64,
        mut on_send: impl FnMut(usize),
        mut on_position: impl FnMut(usize, (f64, f64)),
    ) -> Result<Vec<String>, String> {
        let lines = self.lines.lock().unwrap();
        let mut responses = Vec::new();
        let mut status = StatusPoll::new();
        // Sent and not answered yet: the command's index and its length (with the newline).
        let mut waiting: VecDeque<(usize, usize)> = VecDeque::new();
        let mut buffered = 0;

        for (index, command) in commands.iter().enumerate() {
            let length = command.len() + 1;
            let system = command.starts_with('$');
            while let Some(&(oldest, size)) = waiting.front() {
                if !system && buffered + length <= RX_BUFFER_CHARS {
                    break;
                }
                let accepted = index - waiting.len();
                self.wait_for_ok(&lines, &commands[oldest], generation, &mut responses, &mut status, &mut |p| {
                    on_position(accepted, p)
                })?;
                waiting.pop_front();
                buffered -= size;
            }
            self.check_not_stopped(generation)?;
            on_send(index);
            self.write(format!("{command}
").as_bytes())?;
            waiting.push_back((index, length));
            buffered += length;
            if system {
                self.wait_for_ok(&lines, command, generation, &mut responses, &mut status, &mut |_| {})?;
                waiting.pop_front();
                buffered -= length;
            }
        }
        // The last commands: their "ok"s, so the job is really done.
        while let Some((oldest, _)) = waiting.pop_front() {
            let accepted = oldest;
            self.wait_for_ok(&lines, &commands[oldest], generation, &mut responses, &mut status, &mut |p| {
                on_position(accepted, p)
            })?;
        }
        Ok(responses)
    }

    /// Waits for the "ok" to `command`; other lines received meanwhile go to `responses`.
    /// Meanwhile asks for the status every `STATUS_INTERVAL` and gives each position to `on_position`.
    fn wait_for_ok(
        &self,
        lines: &Receiver<String>,
        command: &str,
        generation: u64,
        responses: &mut Vec<String>,
        status: &mut StatusPoll,
        on_position: &mut dyn FnMut((f64, f64)),
    ) -> Result<(), String> {
        let mut started = Instant::now();
        loop {
            self.check_not_stopped(generation)?;
            if status.asked.elapsed() >= STATUS_INTERVAL {
                status.asked = Instant::now();
                self.write_quiet(&[STATUS_QUERY])?;
            }
            // While paused the laser waits on purpose: don't count that as a timeout.
            if self.is_paused() {
                started = Instant::now();
            }
            match lines.recv_timeout(POLL_INTERVAL) {
                Ok(line) if line == "ok" => return Ok(()),
                // A status report, e.g. "<Run|MPos:10.000,20.000,0.000|FS:1000,0|WCO:0,0,0>".
                Ok(line) if line.starts_with('<') => {
                    if let Some(position) = status.read(&line) {
                        on_position(position);
                    }
                }
                Ok(line) if line.starts_with("error:") || line.starts_with("ALARM:") => {
                    return Err(format!("Laser replied \"{line}\" to command \"{command}\""));
                }
                Ok(line) => responses.push(line),
                Err(RecvTimeoutError::Timeout) if started.elapsed() > COMMAND_TIMEOUT => {
                    return Err(format!("Laser did not respond to command \"{command}\""));
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err("Laser disconnected".to_string()),
            }
        }
    }

    /// Stops the laser immediately (soft reset), then unlocks it with `$X`
    /// so it can accept a new job.
    pub fn stop(&self) -> Result<(), String> {
        self.stop_generation.fetch_add(1, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.write(&[SOFT_RESET])?;

        // Wait for a running job to give up (it releases `lines`)
        // and for GRBL to finish the reset.
        let lines = self.lines.lock().unwrap();
        wait_for_banner(&lines, RESET_TIMEOUT);
        drop(lines);

        self.command("$X")?;
        Ok(())
    }

    /// Pauses the running job (feed hold): motion decelerates to a stop and the
    /// laser goes off (with M4 its power follows the speed). Resume continues it.
    pub fn pause(&self) -> Result<(), String> {
        self.write(&[FEED_HOLD])?;
        self.paused.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Resumes a paused job (cycle start).
    pub fn resume(&self) -> Result<(), String> {
        self.write(&[CYCLE_START])?;
        self.paused.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    fn check_not_stopped(&self, generation: u64) -> Result<(), String> {
        if self.stop_generation() == generation {
            Ok(())
        } else {
            Err("Stopped".to_string())
        }
    }

    /// Like `write`, without printing (the status query goes out several times a second).
    fn write_quiet(&self, bytes: &[u8]) -> Result<(), String> {
        let mut port = self.writer.lock().unwrap();
        port.write_all(bytes)
            .and_then(|_| port.flush())
            .map_err(|e| format!("Cannot send to laser: {e}"))
    }

    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        // Diagnostics: everything we send is printed to the terminal. `escape_ascii`
        // makes control characters visible (Ctrl-X shows as \x18, Enter as \n).
        eprintln!("[{}] -> {}", self.port_name, bytes.escape_ascii());
        let mut port = self.writer.lock().unwrap();
        port.write_all(bytes)
            .and_then(|_| port.flush())
            .map_err(|e| format!("Cannot send to laser: {e}"))
    }
}

/// The status reports of a running job: when the last was asked for, and the work
/// coordinate offset (GRBL sends "WCO" only now and then; MPos − WCO = work position).
struct StatusPoll {
    asked: Instant,
    offset: (f64, f64),
}

impl StatusPoll {
    fn new() -> StatusPoll {
        StatusPoll { asked: Instant::now(), offset: (0.0, 0.0) }
    }

    /// The head's position in work coordinates (those of the G-code), from a status report.
    fn read(&mut self, report: &str) -> Option<(f64, f64)> {
        let fields = report.trim_matches(|c| c == '<' || c == '>').split('|');
        let xy = |values: &str| -> Option<(f64, f64)> {
            let mut numbers = values.split(',').map(|n| n.parse::<f64>());
            Some((numbers.next()?.ok()?, numbers.next()?.ok()?))
        };
        let (mut machine, mut work) = (None, None);
        for field in fields {
            if let Some(values) = field.strip_prefix("WCO:") {
                self.offset = xy(values)?;
            } else if let Some(values) = field.strip_prefix("MPos:") {
                machine = xy(values);
            } else if let Some(values) = field.strip_prefix("WPos:") {
                work = xy(values);
            }
        }
        work.or(machine.map(|(x, y)| (x - self.offset.0, y - self.offset.1)))
    }
}

impl Drop for Grbl {
    fn drop(&mut self) {
        // Stops the reader thread; the port closes once both halves are gone.
        self.alive.store(false, Ordering::SeqCst);
    }
}

/// Runs on a separate thread: reads bytes from the port and forwards them line by line.
fn read_lines(
    mut port: Box<dyn SerialPort>,
    port_name: String,
    tx: Sender<String>,
    alive: Arc<AtomicBool>,
) {
    let mut buffer = [0u8; 256];
    let mut line = Vec::new();

    while alive.load(Ordering::SeqCst) {
        match port.read(&mut buffer) {
            Ok(n) => {
                for &byte in &buffer[..n] {
                    match byte {
                        b'\n' => {
                            let text = String::from_utf8_lossy(&line).trim().to_string();
                            line.clear();
                            // Diagnostics: every line received from the laser.
                            eprintln!("[{port_name}] <- {text}");
                            if !text.is_empty() && tx.send(text).is_err() {
                                alive.store(false, Ordering::SeqCst);
                                return;
                            }
                        }
                        b'\r' => {}
                        _ => line.push(byte),
                    }
                }
            }
            Err(e) if e.kind() == ErrorKind::TimedOut => {}
            Err(e) => {
                eprintln!("[{port_name}] read error: {e}");
                break;
            }
        }
    }
    alive.store(false, Ordering::SeqCst);
}

/// Waits for the GRBL startup message (e.g. "Grbl 1.1h ['$' for help]").
fn wait_for_banner(lines: &Receiver<String>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match lines.recv_timeout(left) {
            Ok(line) if line.starts_with("Grbl") => {
                // Some firmwares (e.g. Mecpow) send an unrequested "ok" after the
                // startup message. If left in the queue, we would take it as the
                // reply to the next command.
                discard_until_quiet(lines);
                return true;
            }
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Discards received lines until the laser has been silent for `QUIET_TIME`.
fn discard_until_quiet(lines: &Receiver<String>) {
    while lines.recv_timeout(QUIET_TIME).is_ok() {}
}
