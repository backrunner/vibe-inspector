<script lang="ts">
  import { Boxes, PanelLeftClose, PanelLeftOpen, Search } from '@lucide/svelte';
  import { Input } from '$lib/components/ui/input';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import IconButton from '$components/IconButton.svelte';
  import ModuleTree from './ModuleTree.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const totalFlowCount = $derived(store.totalFlowCount);
</script>

<aside
  class:collapsed={store.moduleRailCollapsed}
  class="module-rail flex min-w-0 flex-col border-r"
  aria-label="Modules"
>
  {#if store.moduleRailCollapsed}
    <div class="rail-mode rail-mode-collapsed">
      <div class="grid h-11 shrink-0 place-items-center border-b text-muted-foreground">
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
      <header class="flex h-11 shrink-0 items-center justify-between border-b px-4 pr-2">
        <div class="grid min-w-0 gap-0.5">
          <strong class="text-[14px] font-semibold">Architecture</strong><span
            class="truncate text-[11.5px] text-muted-foreground"
            >{store.selectedProject?.name ?? 'No project'}</span
          >
        </div>
        <IconButton
          label="Collapse module rail"
          icon={PanelLeftClose}
          onclick={() => store.toggleModuleRail()}
        />
      </header>

      <div class="module-search relative mx-3 mt-3 mb-2">
        <Search
          class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
          strokeWidth={1.8}
        />
        <Input
          bind:value={store.searchQuery}
          placeholder="Filter architecture"
          aria-label="Filter architecture"
          class="h-8 bg-input/35 pl-8 text-[13px]"
        />
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
    width: 100%;
    overflow: hidden;
    background: var(--module-rail-surface);
  }

  .rail-mode {
    --module-empty-chrome-offset: 124px;

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
