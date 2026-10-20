<script setup lang="ts">
// Laser safety, as LightBurn's "Safety and Warnings": shown at the first start until the user
// ticks "I understand" (saved in settings.json), and any time from Help > Safety.
import { onMounted, onUnmounted, ref } from "vue";
import { X } from "lucide-vue-next";
import { call } from "../laser";

/** firstRun: it must be accepted (no closing with Esc, ✕ or a click outside). */
const props = defineProps<{ firstRun: boolean }>();
const emit = defineEmits<{ close: [] }>();

const understood = ref(false);

function accept() {
  call("accept_safety");
  emit("close");
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape" && !props.firstRun) emit("close");
}
onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="backdrop" @pointerdown.self="!firstRun && emit('close')">
    <div class="dialog">
      <div class="title">
        Laser safety
        <button v-if="!firstRun" class="icon" title="Close (Esc)" @click="emit('close')"><X :size="14" /></button>
      </div>
      <ul>
        <li>Laser light can permanently damage eyes and skin, even when reflected, and it may be invisible.</li>
        <li>
          Everyone in the room wears safety glasses certified (EN 207) for your laser's wavelength (about 450 nm
          for blue diode lasers).
        </li>
        <li>Never leave the laser running unattended.</li>
        <li>Engraving and cutting make smoke and fumes: ventilate the room or use an exhaust.</li>
        <li>Never cut PVC or vinyl: they release chlorine gas, harmful to you and to the machine.</li>
        <li>Materials can catch fire: keep a fire extinguisher at hand.</li>
        <li>To stop a job at once: the Stop button, or the Pause key.</li>
        <li>Your laser's manufacturer instructions come first.</li>
        <li>laserHM is provided as is, without any warranty; you use it at your own risk.</li>
      </ul>
      <div v-if="firstRun" class="footer">
        <label title="Tick to confirm you have read these warnings">
          <input v-model="understood" type="checkbox" /> I understand
        </label>
        <button :disabled="!understood" title="Start using laserHM" @click="accept">OK</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 110;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.3);
}

.dialog {
  width: min(560px, 90vw);
  background: var(--panel);
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
}

.title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 22px;
  padding: 2px 4px 2px 8px;
  border-bottom: 1px solid var(--border);
  font-weight: 600;
  color: var(--danger);
}

ul {
  margin: 0;
  padding: 6px 12px 6px 28px;
  user-select: text;
}

li + li {
  margin-top: 3px;
}

.footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 12px;
  padding: 4px 8px 6px;
  border-top: 1px solid var(--border);
}

label {
  display: flex;
  align-items: center;
  gap: 4px;
}

.footer button {
  padding: 2px 16px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  cursor: pointer;
}

.footer button:disabled {
  opacity: 0.5;
  cursor: default;
}

button.icon {
  display: grid;
  place-items: center;
  padding: 2px;
  border: none;
  border-radius: 4px;
  background: none;
  color: inherit;
  cursor: pointer;
}

button.icon:hover {
  background: var(--hover);
}
</style>
