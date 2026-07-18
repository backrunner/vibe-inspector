<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const shortcuts = [
    { label: 'Toggle project rail', keys: ['⌘', 'B'] },
    { label: 'Filter architecture', keys: ['⌘', 'K'] },
    { label: 'Open settings', keys: ['⌘', ','] },
    { label: 'Show shortcuts', keys: ['⌘', '/'] },
  ];
</script>

<Dialog.Root bind:open={store.shortcutsOpen}>
  <Dialog.Content class="shortcuts-dialog gap-0 overflow-hidden p-0 sm:max-w-[460px]">
    <Dialog.Header class="border-b px-5 py-4 text-left">
      <Dialog.Title class="text-[15px]">Keyboard shortcuts</Dialog.Title>
      <Dialog.Description class="text-[12px]"
        >Navigate the inspector without leaving context.</Dialog.Description
      >
    </Dialog.Header>
    <div class="shortcut-list">
      {#each shortcuts as shortcut (shortcut.label)}
        <div class="shortcut-row">
          <span>{shortcut.label}</span>
          <span class="keys">
            {#each shortcut.keys as key}<kbd>{key}</kbd>{/each}
          </span>
        </div>
      {/each}
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.shortcuts-dialog) {
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--dialog-shadow);
  }

  .shortcut-list {
    display: grid;
    padding: 8px;
  }

  .shortcut-row {
    display: flex;
    min-height: 44px;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    font-size: 12.5px;
  }

  .shortcut-row:hover {
    background: color-mix(in oklab, var(--muted) 62%, transparent);
  }

  .keys {
    display: flex;
    gap: 4px;
  }

  kbd {
    display: grid;
    min-width: 25px;
    height: 23px;
    place-items: center;
    padding: 0 6px;
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-compact);
    color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted) 66%, var(--card));
    box-shadow: var(--control-shadow);
    font-family: var(--app-font-sans);
    font-size: 11px;
    line-height: 1;
  }
</style>
