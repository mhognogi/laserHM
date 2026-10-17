//! Comunicarea cu controlerul GRBL prin portul serial (USB).
//!
//! Protocolul folosit e cel mai simplu din GRBL ("send-response"):
//! trimitem o linie, așteptăm "ok" (sau "error:N"), apoi trimitem următoarea.

use std::io::{ErrorKind, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serialport::SerialPort;

const BAUD_RATE: u32 = 115_200;
/// Ctrl-X: comanda GRBL "soft reset". Oprește imediat mișcarea și laserul.
const SOFT_RESET: u8 = 0x18;
/// Cât așteptăm mesajul de pornire "Grbl ..." după deschiderea portului.
const BANNER_TIMEOUT: Duration = Duration::from_millis(2500);
/// Cât așteptăm mesajul de pornire după un soft reset.
const RESET_TIMEOUT: Duration = Duration::from_millis(1500);
/// Cât timp trebuie să tacă laserul după reset ca să-l considerăm pregătit.
const QUIET_TIME: Duration = Duration::from_millis(300);
/// Cât așteptăm "ok" la o comandă înainte să renunțăm.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// La ce interval verificăm dacă s-a cerut oprirea cât timp așteptăm un răspuns.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

pub struct Grbl {
    port_name: String,
    /// Jumătatea de scriere a portului. Mutex-ul permite ca butonul Stop
    /// să scrie chiar în timp ce o lucrare așteaptă un răspuns.
    writer: Mutex<Box<dyn SerialPort>>,
    /// Liniile primite de la laser, citite de un fir de execuție separat.
    lines: Mutex<Receiver<String>>,
    /// Devine false când portul nu mai răspunde (de ex. cablul a fost scos).
    alive: Arc<AtomicBool>,
    /// Crește la fiecare Stop. O lucrare pornită înainte de Stop vede că
    /// numărul s-a schimbat și se oprește.
    stop_generation: AtomicU64,
}

impl Grbl {
    /// Deschide portul și verifică dacă la celălalt capăt e un controler GRBL.
    pub fn open(port_name: &str) -> Result<Grbl, String> {
        let port = serialport::new(port_name, BAUD_RATE)
            .timeout(POLL_INTERVAL)
            .open()
            .map_err(|e| format!("Nu pot deschide {port_name}: {e}"))?;
        let reader = port
            .try_clone()
            .map_err(|e| format!("Nu pot folosi {port_name}: {e}"))?;

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
        };

        // Multe plăci se resetează la deschiderea portului și trimit singure
        // mesajul de pornire. Dacă nu, le cerem noi un reset.
        let lines = grbl.lines.lock().unwrap();
        let found = wait_for_banner(&lines, BANNER_TIMEOUT) || {
            grbl.write(&[SOFT_RESET])?;
            wait_for_banner(&lines, RESET_TIMEOUT)
        };
        drop(lines);

        if found {
            Ok(grbl)
        } else {
            Err(format!("{port_name} nu răspunde ca un controler GRBL"))
        }
    }

    pub fn port_name(&self) -> &str {
        &self.port_name
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// Valoarea curentă a contorului de Stop; se dă lui `run`.
    pub fn stop_generation(&self) -> u64 {
        self.stop_generation.load(Ordering::SeqCst)
    }

    /// Trimite o singură comandă și întoarce liniile primite înainte de "ok".
    pub fn command(&self, command: &str) -> Result<Vec<String>, String> {
        self.run(&[command.to_string()], self.stop_generation())
    }

    /// Citește o setare GRBL, de ex. `setting(30)` pentru `$30` (puterea maximă S).
    pub fn setting(&self, number: u32) -> Result<f64, String> {
        let prefix = format!("${number}=");
        self.command("$$")?
            .iter()
            .find_map(|line| line.strip_prefix(&prefix)?.trim().parse().ok())
            .ok_or_else(|| format!("Laserul nu a raportat setarea ${number}"))
    }

    /// Trimite comenzile pe rând, fiecare așteptând "ok".
    /// Se oprește dacă între timp s-a apăsat Stop (`generation` nu mai e cel curent).
    pub fn run(&self, commands: &[String], generation: u64) -> Result<Vec<String>, String> {
        let lines = self.lines.lock().unwrap();
        let mut responses = Vec::new();

        for command in commands {
            self.check_not_stopped(generation)?;
            self.write(format!("{command}\n").as_bytes())?;

            let started = Instant::now();
            loop {
                self.check_not_stopped(generation)?;
                match lines.recv_timeout(POLL_INTERVAL) {
                    Ok(line) if line == "ok" => break,
                    Ok(line) if line.starts_with("error:") || line.starts_with("ALARM:") => {
                        return Err(format!("Laserul a răspuns „{line}” la comanda „{command}”"));
                    }
                    Ok(line) => responses.push(line),
                    Err(RecvTimeoutError::Timeout) if started.elapsed() > COMMAND_TIMEOUT => {
                        return Err(format!("Laserul nu a răspuns la comanda „{command}”"));
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err("Laserul s-a deconectat".to_string());
                    }
                }
            }
        }
        Ok(responses)
    }

    /// Oprește imediat laserul (soft reset), apoi îl deblochează cu `$X`
    /// ca să poată primi o nouă lucrare.
    pub fn stop(&self) -> Result<(), String> {
        self.stop_generation.fetch_add(1, Ordering::SeqCst);
        self.write(&[SOFT_RESET])?;

        // Așteptăm ca o eventuală lucrare în curs să renunțe (eliberează `lines`)
        // și ca GRBL să termine resetul.
        let lines = self.lines.lock().unwrap();
        wait_for_banner(&lines, RESET_TIMEOUT);
        drop(lines);

        self.command("$X")?;
        Ok(())
    }

    fn check_not_stopped(&self, generation: u64) -> Result<(), String> {
        if self.stop_generation() == generation {
            Ok(())
        } else {
            Err("Oprit".to_string())
        }
    }

    fn write(&self, bytes: &[u8]) -> Result<(), String> {
        // Diagnostic: tot ce trimitem apare în terminal. `escape_ascii` face vizibile
        // caracterele de control (Ctrl-X apare ca \x18, Enter ca \n).
        eprintln!("[{}] -> {}", self.port_name, bytes.escape_ascii());
        let mut port = self.writer.lock().unwrap();
        port.write_all(bytes)
            .and_then(|_| port.flush())
            .map_err(|e| format!("Nu pot trimite la laser: {e}"))
    }
}

impl Drop for Grbl {
    fn drop(&mut self) {
        // Oprește firul de citire; portul se închide când ambele jumătăți dispar.
        self.alive.store(false, Ordering::SeqCst);
    }
}

/// Rulează pe un fir separat: citește octeți de pe port și îi trimite linie cu linie.
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
                            // Diagnostic: fiecare linie primită de la laser.
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
                eprintln!("[{port_name}] eroare la citire: {e}");
                break;
            }
        }
    }
    alive.store(false, Ordering::SeqCst);
}

/// Așteaptă mesajul de pornire GRBL (de ex. "Grbl 1.1h ['$' for help]").
fn wait_for_banner(lines: &Receiver<String>, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match lines.recv_timeout(left) {
            Ok(line) if line.starts_with("Grbl") => {
                // Unele firmware-uri (de ex. Mecpow) trimit după mesajul de pornire
                // și un "ok" pe care nu l-am cerut. Dacă l-am lăsa în coadă, l-am
                // lua drept răspuns la următoarea comandă.
                discard_until_quiet(lines);
                return true;
            }
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    false
}

/// Aruncă liniile primite până când laserul tace `QUIET_TIME`.
fn discard_until_quiet(lines: &Receiver<String>) {
    while lines.recv_timeout(QUIET_TIME).is_ok() {}
}
