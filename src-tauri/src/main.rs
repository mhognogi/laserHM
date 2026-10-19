// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `laserhm.exe --mcp`: started by Claude Desktop, no window (see mcp.rs).
    if std::env::args().any(|arg| arg == "--mcp") {
        laserhm_lib::mcp::run_bridge();
        return;
    }
    laserhm_lib::run()
}
