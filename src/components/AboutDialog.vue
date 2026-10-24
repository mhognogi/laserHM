<script setup lang="ts">
// Help > About: app name, version and author, centered over the window.
import { onMounted, onUnmounted, ref } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import icon from "../../src-tauri/icons/64x64.png";

const emit = defineEmits<{ close: [] }>();

/** The version from tauri.conf.json; empty until Tauri answers. */
const version = ref("");

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape" || event.key === "Enter") emit("close");
}

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  version.value = await getVersion();
});
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <!-- A click outside the box closes it. -->
  <div class="backdrop" @pointerdown.self="emit('close')">
    <div class="about">
      <img :src="icon" alt="" />
      <div class="name">LaserHM</div>
      <div>Version {{ version }}</div>
      <div class="muted">Design and cutting for GRBL diode lasers</div>
      <div class="muted">© 2026 Marius Hognogi</div>
      <div class="muted">Built with Tauri, Rust and Vue</div>
      <button title="Close (Esc or Enter)" @click="emit('close')">OK</button>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.3);
}

.about {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  padding: 12px 24px;
  background: var(--panel);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.name {
  font-size: 1.4em;
  font-weight: 600;
}

.muted {
  color: var(--muted);
}

button {
  margin-top: 8px;
  padding: 2px 16px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  cursor: pointer;
}

button:hover {
  background: var(--hover);
}
</style>
