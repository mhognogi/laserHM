<script setup lang="ts">
import MenuBar from "./components/MenuBar.vue";
import NumericBar from "./components/NumericBar.vue";
import ToolBar from "./components/ToolBar.vue";
import WorkArea from "./components/WorkArea.vue";
import LaserPanel from "./components/LaserPanel.vue";
</script>

<template>
  <div class="layout">
    <MenuBar class="full-width" />
    <NumericBar class="full-width" />
    <ToolBar />
    <WorkArea />
    <LaserPanel />
  </div>
</template>

<style scoped>
/* Row 1: the menu across the full width.
   Row 2: position and size of the selected shape, across the full width.
   Row 3: tools | work area | laser panel. */
.layout {
  display: grid;
  grid-template-rows: auto auto 1fr;
  grid-template-columns: auto 1fr 260px;
  height: 100vh;
}

.full-width {
  grid-column: 1 / -1;
}
</style>
