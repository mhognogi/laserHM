import { createApp } from "vue";
import App from "./App.vue";
import "./styles.css";
// Claude Desktop can draw on the canvas (tool calls arrive from Rust).
import "./mcp";
import { startSession } from "./project";

// Create the Vue app and mount it into <div id="app"> from index.html.
createApp(App).mount("#app");

// The canvas comes back as it was when the app was closed.
startSession();
