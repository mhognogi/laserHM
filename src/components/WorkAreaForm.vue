<script setup lang="ts">
import { computed, ref } from "vue";
import { setWorkArea, type WorkArea } from "../laser";

// Props: values given by the parent component, e.g. <WorkAreaForm submit-label="Set" />.
const props = defineProps<{
  /** Values to pre-fill the inputs with. */
  initial?: WorkArea | null;
  submitLabel: string;
  /** Required: every actionable element has an explanatory title. */
  submitTitle: string;
}>();

// Events this component sends to its parent, e.g. <WorkAreaForm @saved="...">.
const emit = defineEmits<{ saved: [] }>();

const width = ref(props.initial?.width);
const height = ref(props.initial?.height);
const error = ref("");
// An empty input gives "" instead of a number, and Number("") is 0,
// so the button stays disabled until both fields hold a positive value.
const canSave = computed(() => Number(width.value) > 0 && Number(height.value) > 0);

async function save() {
  try {
    await setWorkArea(Number(width.value), Number(height.value));
    error.value = "";
    emit("saved");
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <!-- @submit.prevent: handle the form ourselves instead of reloading the page. -->
  <form class="work-area-form" @submit.prevent="save">
    <!-- v-model.number: two-way link between the input and the variable, as a number. -->
    <input
      v-model.number="width"
      type="number"
      min="1"
      step="any"
      required
      placeholder="width"
      aria-label="Width"
      title="Work area width in mm (X axis)"
    />
    <input
      v-model.number="height"
      type="number"
      min="1"
      step="any"
      required
      placeholder="height"
      aria-label="Height"
      title="Work area height in mm (Y axis)"
    />
    <button type="submit" :disabled="!canSave" :title="submitTitle">{{ submitLabel }}</button>
    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<style scoped>
/* All elements on one row; the error message (if any) wraps below. */
.work-area-form {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.error {
  flex-basis: 100%;
  margin: 0;
  text-align: center;
  color: var(--danger);
}

input {
  /* ch = width of the digit "0" in the current font; 7ch fits the
     whole "height" placeholder (letters are not all as wide as digits). */
  width: 7ch;
  padding: 2px 4px;
  border: 1px solid var(--border-strong);
  border-radius: 4px;
  background: var(--button);
  color: inherit;
  font: inherit;
  /* Hide the up/down arrows of number inputs. */
  appearance: textfield;
}

/* The arrows in Chromium-based engines (WebView2 used by Tauri on Windows). */
input::-webkit-inner-spin-button,
input::-webkit-outer-spin-button {
  -webkit-appearance: none;
  margin: 0;
}

button {
  padding: 2px 10px;
  border: 1px solid var(--accent);
  border-radius: 4px;
  background: var(--accent);
  color: #ffffff;
  font: inherit;
  cursor: pointer;
}

button:disabled {
  opacity: 0.4;
  cursor: default;
}
</style>
