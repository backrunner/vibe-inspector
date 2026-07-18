<script lang="ts">
  import { Boxes, PanelLeftClose, PanelLeftOpen, Search } from '@lucide/svelte';
  import { Input } from '$lib/components/ui/input';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import IconButton from '$components/IconButton.svelte';
  import LiquidGlassSurface from '$components/LiquidGlassSurface.svelte';
  import ModuleTree from './ModuleTree.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const totalFlowCount = $derived(store.totalFlowCount);
</script>

<aside
  class:collapsed={store.moduleRailCollapsed}
  class="module-rail flex min-w-0 flex-col"
  aria-label="Modules"
>
  {#if store.moduleRailCollapsed}
    <div class="rail-mode rail-mode-collapsed">
      <div class="rail-header grid h-11 shrink-0 place-items-center text-muted-foreground">
        <IconButton
          label="Expand module rail"
          icon={PanelLeftOpen}
          onclick={() => store.toggleModuleRail()}
        />
      </div>
      <Boxes
        class="mx-auto mt-4 size-4 text-muted-foreground"
        strokeWidth={1.7}
        aria-hidden="true"
      />
    </div>
  {:else}
    <div class="rail-mode rail-mode-expanded">
      <header class="rail-header architecture-header">
        <div class="architecture-title">
          <strong>Architecture</strong><span>{store.selectedProject?.name ?? 'No project'}</span>
        </div>
        <IconButton
          label="Collapse module rail"
          icon={PanelLeftClose}
          onclick={() => store.toggleModuleRail()}
        />
      </header>

      <div class="module-search mx-3 mt-3 mb-2">
        <LiquidGlassSurface variant="field" class="module-search-glass">
          <Search class="search-icon" strokeWidth={1.8} aria-hidden="true" />
          <Input
            bind:value={store.searchQuery}
            placeholder="Filter architecture"
            aria-label="Filter architecture"
            class="h-8 border-0 bg-transparent pr-3 pl-8 text-[13px] [box-shadow:none] focus-visible:border-transparent focus-visible:ring-0 dark:bg-transparent"
          />
        </LiquidGlassSurface>
      </div>

      <div
        class="flex items-center justify-between px-4 pt-1 pb-2 text-[11px] text-muted-foreground"
      >
        <span>Modules</span>
        <span class="font-mono">{totalFlowCount} {totalFlowCount === 1 ? 'flow' : 'flows'}</span>
      </div>

      <ScrollArea class="min-h-0 flex-1"><ModuleTree /></ScrollArea>
    </div>
  {/if}
</aside>

<style>
  .module-rail {
    position: relative;
    z-index: 1;
    width: 100%;
    overflow: hidden;
    border: 1px solid var(--panel-edge);
    border-radius: var(--radius-xl);
    background: var(--module-rail-surface);
    box-shadow: var(--panel-shadow);
  }

  .rail-header {
    position: relative;
    background: var(--rail-header-surface);
    box-shadow: var(--rail-header-shadow);
  }

  .rail-header::after {
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 12px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .architecture-header {
    display: flex;
    min-height: 56px;
    flex: 0 0 56px;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 0 8px 0 16px;
  }

  .architecture-title {
    display: grid;
    min-width: 0;
    gap: 5px;
  }

  .architecture-title strong,
  .architecture-title span {
    overflow: hidden;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .architecture-title strong {
    font-size: 14px;
    font-weight: 620;
  }

  .architecture-title span {
    color: var(--muted-foreground);
    font-size: 11.5px;
  }

  .module-search {
    position: relative;
  }

  :global(.module-search-glass .search-icon) {
    position: absolute;
    top: 50%;
    left: 10px;
    z-index: 2;
    width: 14px;
    height: 14px;
    color: var(--muted-foreground);
    pointer-events: none;
    transform: translateY(-50%);
  }

  :global(.module-search-glass:has(input:focus-visible) .search-icon) {
    color: var(--foreground);
  }

  .rail-mode {
    --module-empty-chrome-offset: 136px;

    display: flex;
    width: 280px;
    min-width: 280px;
    height: 100%;
    flex-direction: column;
    animation: rail-content-in var(--dur-rail) var(--ease-out) both;
  }

  .rail-mode-collapsed {
    width: 48px;
    min-width: 48px;
  }

  @keyframes rail-content-in {
    from {
      opacity: 0;
      transform: translateX(-6px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  @media (max-width: 1200px) {
    .rail-mode-expanded {
      width: 252px;
      min-width: 252px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .rail-mode {
      animation: none;
    }
  }
</style>
