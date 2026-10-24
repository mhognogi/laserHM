// Projects: File > New / Open / Save / Save As (.lhm files), and the last session, saved
// automatically and restored when the app starts. A project is JSON text: the shapes and
// the pictures they use (as data URLs, so the file is complete on its own).
import { nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { editor, replaceShapes, type Shape } from "./editor";
import { imagesOf, restoreImages } from "./images";
import { resetHistory } from "./history";
import { status } from "./laser";

/** The content of a .lhm file. */
interface ProjectFile {
  format: "LaserHM project";
  version: 1;
  shapes: Shape[];
  /** Pictures by the id the image shapes use. */
  images: Record<number, string>;
}

/** The last session also remembers which file it came from, and whether it was saved. */
interface SessionFile extends ProjectFile {
  path: string | null;
  unsaved: boolean;
}

const FILTERS = [{ name: "LaserHM project", extensions: ["lhm"] }];

/** The open project's file, or null for a new one never saved. */
export const projectPath = ref<string | null>(null);
/** Changed since the last Save or Open. */
export const unsaved = ref(false);

/** True while a project is being put in place, so that does not count as a change. */
let loading = false;

function content(): ProjectFile {
  return { format: "LaserHM project", version: 1, shapes: editor.shapes, images: imagesOf(editor.shapes) };
}

/** Puts a project's shapes and pictures in place of the current ones. */
async function load(file: ProjectFile, path: string | null) {
  if (file.format !== "LaserHM project") throw "This is not a LaserHM project";
  loading = true;
  await restoreImages(file.shapes, file.images ?? {});
  replaceShapes(file.shapes);
  resetHistory();
  projectPath.value = path;
  // The change watcher runs after this; only then is the project "unchanged".
  await nextTick();
  loading = false;
}

function showError(error: unknown) {
  status.message = String(error);
  status.error = true;
}

/** Unsaved changes are lost only if the user agrees. */
async function mayDiscard(): Promise<boolean> {
  if (!unsaved.value) return true;
  return ask("The changes to this project are not saved. Discard them?", {
    title: "LaserHM",
    kind: "warning",
  });
}

/** File > New: an empty canvas. */
export async function newProject() {
  if (!(await mayDiscard())) return;
  await load({ format: "LaserHM project", version: 1, shapes: [], images: {} }, null);
  unsaved.value = false;
}

/** File > Open (Ctrl+O). */
export async function openProject() {
  if (!(await mayDiscard())) return;
  const path = await open({ filters: FILTERS, multiple: false, directory: false });
  if (!path) return;
  try {
    await load(JSON.parse(await invoke<string>("open_project", { path })), path);
    unsaved.value = false;
  } catch (error) {
    showError(error);
  }
}

/** File > Save (Ctrl+S): to the project's file, or asks for one the first time. */
export async function saveProject() {
  if (!projectPath.value) return saveProjectAs();
  try {
    await invoke("save_project", { path: projectPath.value, content: JSON.stringify(content()) });
    unsaved.value = false;
  } catch (error) {
    showError(error);
  }
}

/** File > Save As (Ctrl+Shift+S): to a new file. */
export async function saveProjectAs() {
  const path = await save({ filters: FILTERS, defaultPath: projectPath.value ?? "project.lhm" });
  if (!path) return;
  projectPath.value = path;
  await saveProject();
}

// ----- Window title -----

/** "LaserHM 0.1.0 - name.lhm*": the project's name, * when it has unsaved changes. */
async function updateTitle() {
  const version = await getVersion();
  const name = projectPath.value?.split(/[\\/]/).pop() ?? "Untitled";
  await getCurrentWindow().setTitle(`LaserHM ${version} - ${name}${unsaved.value ? "*" : ""}`);
}
watch([projectPath, unsaved], updateTitle);

// ----- Last session -----

/** How long after the last change the session is saved. */
const SESSION_DELAY_MS = 1000;
let sessionTimer = 0;

function saveSession() {
  const session: SessionFile = { ...content(), path: projectPath.value, unsaved: unsaved.value };
  invoke("save_session", { content: JSON.stringify(session) }).catch(showError);
}

/** Restores the last session, then saves it again after every change. Called once at startup. */
export async function startSession() {
  try {
    const text = await invoke<string | null>("load_session");
    if (text) {
      const session = JSON.parse(text) as SessionFile;
      await load(session, session.path);
      unsaved.value = !!session.unsaved;
    }
  } catch (error) {
    // A damaged session is skipped: the app starts empty.
    console.error(error);
  }
  watch(
    () => editor.shapes,
    () => {
      if (loading) return;
      unsaved.value = true;
      clearTimeout(sessionTimer);
      sessionTimer = window.setTimeout(saveSession, SESSION_DELAY_MS);
    },
    { deep: true },
  );
  // Saved also when only the project's file changes (Save As), and with it whether it is saved.
  watch([projectPath, unsaved], () => {
    clearTimeout(sessionTimer);
    sessionTimer = window.setTimeout(saveSession, SESSION_DELAY_MS);
  });
  updateTitle();
}
