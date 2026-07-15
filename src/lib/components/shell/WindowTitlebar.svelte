<script lang="ts">
  import { History, Moon, Settings2, Sun } from '@lucide/svelte';
  import IconButton from '$components/IconButton.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
</script>

<header class="window-titlebar" data-tauri-drag-region>
  <strong data-tauri-drag-region>VibeInspector</strong>
  <div class="titlebar-actions">
    {#if store.selectedProject}
      <IconButton label="Open activity" icon={History} onclick={() => void store.openActivity()} />
    {/if}
    <IconButton label="Open settings" icon={Settings2} onclick={() => store.openSettings()} />
    <IconButton
      label={store.theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
      icon={store.theme === 'dark' ? Sun : Moon}
      onclick={() => store.toggleTheme()}
    />
  </div>
</header>

<style>
  .window-titlebar {
    position: relative;
    display: grid;
    width: 100%;
    height: 38px;
    flex: 0 0 38px;
    place-items: center;
    background: transparent;
    -webkit-app-region: drag;
    user-select: none;
  }

  .window-titlebar > strong {
    color: color-mix(in oklab, var(--foreground) 82%, transparent);
    font-size: 13px;
    font-weight: 590;
    line-height: 1;
  }

  .titlebar-actions {
    position: absolute;
    top: 5px;
    right: 8px;
    display: flex;
    gap: 2px;
    -webkit-app-region: no-drag;
  }
</style>
