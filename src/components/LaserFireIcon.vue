<script setup lang="ts">
// A laser beam hitting a surface, drawn like the lucide icons (24×24, 2px round strokes,
// the text color), so it matches the other tool icons.
defineProps<{ size?: number }>();
</script>

<template>
  <svg
    xmlns="http://www.w3.org/2000/svg"
    :width="size ?? 24"
    :height="size ?? 24"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <!-- The beam, coming down. -->
    <line x1="12" y1="2" x2="12" y2="16" />
    <!-- The surface it hits. -->
    <line x1="3" y1="20" x2="21" y2="20" />
    <!-- Sparks at the point of impact. -->
    <line x1="8.5" y1="17" x2="6" y2="14.5" />
    <line x1="15.5" y1="17" x2="18" y2="14.5" />
  </svg>
</template>
