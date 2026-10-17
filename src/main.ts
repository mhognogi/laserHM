import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** Aceeași formă ca structura `Status` din src-tauri/src/state.rs. */
interface Status {
  connected: boolean;
  busy: boolean;
  message: string;
}

const statusEl = document.querySelector<HTMLParagraphElement>("#status")!;
const startButton = document.querySelector<HTMLButtonElement>("#start")!;
const stopButton = document.querySelector<HTMLButtonElement>("#stop")!;

function show(status: Status) {
  statusEl.textContent = status.message;
  startButton.disabled = !status.connected || status.busy;
  stopButton.disabled = !status.connected;
}

/** Apelează o comandă Rust și afișează eroarea, dacă apare. */
async function call(command: string) {
  try {
    await invoke(command);
  } catch (error) {
    statusEl.textContent = String(error);
  }
}

startButton.addEventListener("click", () => call("start"));
stopButton.addEventListener("click", () => call("stop"));

// Rust anunță fiecare schimbare de stare; la pornire cerem starea curentă.
await listen<Status>("laser-status", (event) => show(event.payload));
show(await invoke<Status>("get_status"));
