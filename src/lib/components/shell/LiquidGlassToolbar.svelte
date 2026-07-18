<script lang="ts">
  import type { Snippet } from 'svelte';
  import LiquidGlassSurface from '$components/LiquidGlassSurface.svelte';

  let { children } = $props<{ children: Snippet }>();
</script>

<LiquidGlassSurface class="liquid-glass-toolbar" role="group" ariaLabel="Global controls">
  {@render children()}
</LiquidGlassSurface>

<style>
  :global(.liquid-glass-toolbar button) {
    color: color-mix(in oklab, var(--foreground) 84%, transparent);
  }

  :global(.liquid-glass-toolbar button:hover) {
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
  }
</style>
