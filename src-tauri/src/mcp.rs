//! MCP (Model Context Protocol): lets Claude Desktop draw on LaserHM's canvas.
//!
//! Claude Desktop starts `laserhm.exe --mcp` (the bridge, `run_bridge`) and talks to it
//! over stdin/stdout: JSON-RPC 2.0, one message per line. The bridge forwards each tool
//! call to the running LaserHM over a local TCP connection (127.0.0.1 only), proving
//! itself with the secret token LaserHM wrote to mcp.json in its config folder.
//! LaserHM hands the call to the UI (event "mcp-request"), which changes the canvas
//! and answers through `mcp_respond` (see src/mcp.ts).
//!
//! The tools only change the drawing: the laser itself (Start, Fire, Home, moves)
//! stays in the user's hands.

use std::collections::HashMap;
use std::fs;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, State};

/// Name of the event that passes a tool call to the UI.
const REQUEST_EVENT: &str = "mcp-request";
/// How long a tool call may take in the UI (the first text loads the fonts, which is slow).
const ANSWER_TIMEOUT: Duration = Duration::from_secs(60);
/// The app's identifier (tauri.conf.json): its config folder is named after it (see `app_config_dir`).
const IDENTIFIER: &str = "com.mhogn.laserhm";

/// Where the running LaserHM listens, and its secret; in mcp.json.
#[derive(Serialize, Deserialize)]
struct Endpoint {
    port: u16,
    token: String,
}

/// A tool call, from the bridge to LaserHM (one JSON line).
#[derive(Serialize, Deserialize)]
struct ToolCall {
    token: String,
    tool: String,
    arguments: Value,
}

/// The answer to a tool call: a text for Claude, and whether it is an error.
#[derive(Serialize, Deserialize)]
struct ToolAnswer {
    ok: bool,
    text: String,
}

/// LaserHM's config folder, found without Tauri (the bridge has no app), as Tauri finds it:
/// %APPDATA%\<identifier>.
fn app_config_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join(IDENTIFIER))
}

fn endpoint_file(config_dir: PathBuf) -> PathBuf {
    config_dir.join("mcp.json")
}

// ----- In LaserHM: the local server -----

/// The tool calls waiting for the UI's answer, by number.
#[derive(Default)]
pub struct McpState {
    pending: Mutex<HashMap<u64, mpsc::Sender<ToolAnswer>>>,
    next_id: AtomicU64,
}

/// A secret that is hard to guess: hashes with the random keys Rust picks for each
/// `RandomState`, mixed with the time.
fn new_token() -> String {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default().as_nanos();
    (0..2)
        .map(|i| {
            let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
            hasher.write_u128(now + i);
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// Starts listening for the bridge on a free local port and writes mcp.json.
/// Without it LaserHM works as before, only Claude Desktop cannot reach it.
pub fn start(app: AppHandle) {
    let Ok(config_dir) = app.path().app_config_dir() else { return };
    let Ok(listener) = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)) else { return };
    let Ok(address) = listener.local_addr() else { return };
    let endpoint = Endpoint { port: address.port(), token: new_token() };
    let _ = fs::create_dir_all(&config_dir);
    let Ok(text) = serde_json::to_string(&endpoint) else { return };
    if fs::write(endpoint_file(config_dir), text).is_err() {
        return;
    }
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            let token = endpoint.token.clone();
            thread::spawn(move || serve(&app, stream, &token));
        }
    });
}

/// Answers the tool calls of one bridge connection, one line each.
fn serve(app: &AppHandle, stream: TcpStream, token: &str) {
    let Ok(mut writer) = stream.try_clone() else { return };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { return };
        let answer = match serde_json::from_str::<ToolCall>(&line) {
            Ok(call) if call.token == token => ask_ui(app, call),
            // A wrong token: not our bridge; stop talking to it.
            Ok(_) => return,
            Err(error) => ToolAnswer { ok: false, text: format!("Invalid request: {error}") },
        };
        let Ok(text) = serde_json::to_string(&answer) else { return };
        if writeln!(writer, "{text}").is_err() {
            return;
        }
    }
}

/// Passes the call to the UI and waits for its answer.
fn ask_ui(app: &AppHandle, call: ToolCall) -> ToolAnswer {
    let state = app.state::<McpState>();
    let id = state.next_id.fetch_add(1, Ordering::SeqCst);
    let (sender, receiver) = mpsc::channel();
    state.pending.lock().unwrap().insert(id, sender);
    let request = json!({ "id": id, "tool": call.tool, "arguments": call.arguments });
    let answer = match app.emit(REQUEST_EVENT, request) {
        Ok(()) => receiver.recv_timeout(ANSWER_TIMEOUT).unwrap_or(ToolAnswer {
            ok: false,
            text: "LaserHM did not answer in time".into(),
        }),
        Err(error) => ToolAnswer { ok: false, text: error.to_string() },
    };
    state.pending.lock().unwrap().remove(&id);
    answer
}

/// The UI's answer to tool call `id`.
#[tauri::command]
pub fn mcp_respond(state: State<McpState>, id: u64, ok: bool, text: String) {
    if let Some(sender) = state.pending.lock().unwrap().remove(&id) {
        let _ = sender.send(ToolAnswer { ok, text });
    }
}

// ----- `laserhm.exe --mcp`: the bridge started by Claude Desktop -----

/// What Claude reads first: how the tools describe the drawing.
const INSTRUCTIONS: &str = "LaserHM is a laser cutter/engraver app. These tools draw on its canvas, \
which must be open on this computer. Coordinates are in mm on the machine's work area: origin at the \
bottom-left corner, X to the right, Y up. Angles are in degrees, counterclockwise. Shapes are outlines \
(the laser follows them). Call get_canvas first to see the work area and the shapes already there. \
Every change can be undone in LaserHM with Ctrl+Z. The tools never run the laser: cutting is started \
only by the user.";

/// The tools Claude can use; the UI runs them (src/mcp.ts).
fn tools() -> Value {
    let ids = json!({ "type": "array", "items": { "type": "integer" }, "description": "Shape ids (from get_canvas)" });
    json!([
        {
            "name": "get_canvas",
            "description": "The work area's size, all shapes (id, type, center cx/cy, width, height, angle, text) and the selected ids.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "add_shape",
            "description": "Adds a rectangle, ellipse (a circle when width = height) or polygon (a hexagon filling its box), given by its center, size and rotation. Returns its id; it becomes the selection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["rectangle", "ellipse", "polygon"] },
                    "cx": { "type": "number", "description": "Center X, mm" },
                    "cy": { "type": "number", "description": "Center Y, mm" },
                    "width": { "type": "number", "description": "mm, > 0" },
                    "height": { "type": "number", "description": "mm, > 0" },
                    "angle": { "type": "number", "description": "Rotation around the center, degrees counterclockwise; default 0" }
                },
                "required": ["type", "cx", "cy", "width", "height"]
            }
        },
        {
            "name": "add_text",
            "description": "Adds a text; its first line starts at (x, y), on its baseline. '\\n' starts a new line. Returns its id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "x": { "type": "number", "description": "mm" },
                    "y": { "type": "number", "description": "mm" },
                    "height": { "type": "number", "description": "Height of the capital letters, mm; default: the size set in LaserHM" },
                    "font": { "type": "string", "description": "Installed font name (see list_fonts); default: the font set in LaserHM" },
                    "welded": { "type": "boolean", "description": "Join overlapping letters into one outline (for script fonts); default: as set in LaserHM" }
                },
                "required": ["text", "x", "y"]
            }
        },
        {
            "name": "list_fonts",
            "description": "The names of the fonts installed on this computer.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "add_path",
            "description": "Adds a line through the points, with straight segments; closed: also from the last point back to the first. Returns its id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "points": {
                        "type": "array",
                        "minItems": 2,
                        "items": {
                            "type": "object",
                            "properties": { "x": { "type": "number" }, "y": { "type": "number" } },
                            "required": ["x", "y"]
                        }
                    },
                    "closed": { "type": "boolean" }
                },
                "required": ["points", "closed"]
            }
        },
        {
            "name": "select",
            "description": "Selects these shapes in LaserHM (an empty list deselects all).",
            "inputSchema": { "type": "object", "properties": { "ids": ids }, "required": ["ids"] }
        },
        {
            "name": "move",
            "description": "Moves the shapes by dx, dy mm.",
            "inputSchema": {
                "type": "object",
                "properties": { "ids": ids, "dx": { "type": "number" }, "dy": { "type": "number" } },
                "required": ["ids", "dx", "dy"]
            }
        },
        {
            "name": "set_geometry",
            "description": "Changes one shape's center, size or rotation; the fields left out stay as they are.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "integer" },
                    "cx": { "type": "number" },
                    "cy": { "type": "number" },
                    "width": { "type": "number" },
                    "height": { "type": "number" },
                    "angle": { "type": "number" }
                },
                "required": ["id"]
            }
        },
        {
            "name": "rotate",
            "description": "Turns the shapes together around the center of the box around them, by degrees (counterclockwise; negative: clockwise).",
            "inputSchema": {
                "type": "object",
                "properties": { "ids": ids, "degrees": { "type": "number" } },
                "required": ["ids", "degrees"]
            }
        },
        {
            "name": "duplicate",
            "description": "Copies the shapes, moved by dx, dy mm. Returns the new ids.",
            "inputSchema": {
                "type": "object",
                "properties": { "ids": ids, "dx": { "type": "number" }, "dy": { "type": "number" } },
                "required": ["ids", "dx", "dy"]
            }
        },
        {
            "name": "delete",
            "description": "Deletes the shapes.",
            "inputSchema": { "type": "object", "properties": { "ids": ids }, "required": ["ids"] }
        },
        {
            "name": "boolean",
            "description": "Combines shapes into one (they are replaced by the result): weld joins 2 or more into one outline; subtract removes the second id from the first; intersect keeps what the 2 have in common. Closed shapes only. Returns the result's id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "op": { "type": "string", "enum": ["weld", "subtract", "intersect"] },
                    "ids": ids
                },
                "required": ["op", "ids"]
            }
        },
        {
            "name": "break_apart",
            "description": "Splits shapes made of separate pieces (e.g. a welded text) into one shape per piece; holes stay with their piece. Returns the new ids.",
            "inputSchema": { "type": "object", "properties": { "ids": ids }, "required": ["ids"] }
        }
    ])
}

/// Sends a tool call to the running LaserHM and returns its answer.
fn call_laserhm(tool: &str, arguments: Value) -> Result<ToolAnswer, String> {
    let not_running = || "LaserHM is not running: open it, then try again".to_string();
    let config_dir = app_config_dir().ok_or_else(not_running)?;
    let text = fs::read_to_string(endpoint_file(config_dir)).map_err(|_| not_running())?;
    let endpoint: Endpoint = serde_json::from_str(&text).map_err(|_| not_running())?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, endpoint.port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|_| not_running())?;
    stream.set_read_timeout(Some(ANSWER_TIMEOUT + Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let call = ToolCall { token: endpoint.token, tool: tool.to_string(), arguments };
    let line = serde_json::to_string(&call).map_err(|e| e.to_string())?;
    writeln!(stream, "{line}").map_err(|_| not_running())?;
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).map_err(|_| not_running())?;
    serde_json::from_str(&answer).map_err(|_| not_running())
}

/// The result of `tools/call`: the answer as text; errors are marked, so Claude sees them.
fn call_tool(params: &Value) -> Value {
    let name = params["name"].as_str().unwrap_or_default();
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
    let answer = call_laserhm(name, arguments).unwrap_or_else(|text| ToolAnswer { ok: false, text });
    json!({ "content": [{ "type": "text", "text": answer.text }], "isError": !answer.ok })
}

/// Runs the bridge until Claude Desktop closes stdin.
pub fn run_bridge() {
    let mut out = std::io::stdout();
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else { continue };
        // Only requests and notifications have a method (we never send requests ourselves).
        let Some(method) = message["method"].as_str() else { continue };
        let result = match method {
            "initialize" => Ok(json!({
                // Answer with the version Claude Desktop asked for.
                "protocolVersion": message["params"]["protocolVersion"],
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "LaserHM", "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS
            })),
            "tools/list" => Ok(json!({ "tools": tools() })),
            "tools/call" => Ok(call_tool(&message["params"])),
            "ping" => Ok(json!({})),
            _ => Err(json!({ "code": -32601, "message": format!("Method not found: {method}") })),
        };
        // Notifications (e.g. "notifications/initialized") have no id and get no answer.
        let Some(id) = message.get("id") else { continue };
        let reply = match result {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        if writeln!(out, "{reply}").and_then(|_| out.flush()).is_err() {
            break;
        }
    }
}
