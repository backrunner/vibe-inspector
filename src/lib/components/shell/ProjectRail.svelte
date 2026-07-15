<script lang="ts">
  import { FolderPlus, PanelLeftClose, PanelLeftOpen } from '@lucide/svelte';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import IconButton from '$components/IconButton.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
</script>

<aside
  class:collapsed={store.projectRailCollapsed}
  class="project-rail flex min-w-0 flex-col border-r border-sidebar-border text-sidebar-foreground"
  aria-label="Projects"
>
  {#if store.projectRailCollapsed}
    <div class="grid h-11 shrink-0 place-items-center border-b border-sidebar-border">
      <IconButton
        label="Expand project rail"
        icon={PanelLeftOpen}
        onclick={() => store.toggleProjectRail()}
      />
    </div>
    <div class="grid justify-items-center gap-2 py-3">
      {#each store.projects as project}
        <button
          class:active={project.id === store.selectedProjectId}
          class="project-avatar relative grid size-8 place-items-center rounded-md border border-transparent font-mono text-[9px] font-semibold transition-colors"
          aria-label={`Open ${project.name}`}
          title={project.name}
          onclick={() => store.selectProject(project.id)}
          >{project.name.slice(0, 2).toUpperCase()}<span
            class:stale={project.status === 'stale'}
            class:error={project.status === 'error'}
            class:syncing={project.status === 'syncing'}
            class="collapsed-status"
            aria-hidden="true"
          ></span></button
        >
      {/each}
    </div>
  {:else}
    <div
      class="flex h-11 shrink-0 items-center justify-between border-b border-sidebar-border px-2 pl-4"
    >
      <span class="text-[13px] font-semibold">Projects</span>
      <div class="flex items-center">
        <IconButton
          label="Collapse project rail"
          icon={PanelLeftClose}
          onclick={() => store.toggleProjectRail()}
        />
        <IconButton
          label="Register a project"
          icon={FolderPlus}
          onclick={() => store.openProjectPicker()}
        />
      </div>
    </div>

    <ScrollArea class="min-h-0 flex-1">
      <div class="grid gap-0.5 px-2">
        {#each store.projects as project}
          <button
            class:active={project.id === store.selectedProjectId}
            class="project-row group flex min-w-0 items-center gap-2.5 rounded-md border border-transparent py-2.5 pl-2 pr-[11px] text-left transition-colors hover:bg-sidebar-accent"
            aria-current={project.id === store.selectedProjectId ? 'page' : undefined}
            onclick={() => store.selectProject(project.id)}
          >
            <span
              class="project-mark grid size-7 shrink-0 place-items-center rounded-md font-mono text-[9px] font-semibold"
              >{project.name.slice(0, 2).toUpperCase()}</span
            >
            <span class="grid min-w-0 flex-1 gap-0.5"
              ><strong class="truncate text-[13px] font-medium">{project.name}</strong><small
                class="truncate font-mono text-[10.5px] text-muted-foreground"
                >{project.branch} · {project.lastScanAt}</small
              ></span
            >
            <span
              class:stale={project.status === 'stale'}
              class:error={project.status === 'error'}
              class:syncing={project.status === 'syncing'}
              class="status-dot size-1.5 shrink-0 rounded-full"
              title={project.status}
              aria-label={`Project status: ${project.status}`}
            ></span>
          </button>
        {/each}
      </div>
    </ScrollArea>
  {/if}
</aside>

<style>
  .project-rail {
    flex: 0 0 216px;
    background: var(--project-rail-surface);
    transition: flex-basis 160ms ease;
  }
  .project-rail.collapsed {
    flex-basis: 56px;
  }
  .project-row.active {
    border-color: var(--sidebar-border);
    background: var(--sidebar-accent);
    box-shadow: inset 0 1px 0 color-mix(in oklab, var(--foreground) 4%, transparent);
  }
  .project-avatar.active {
    border-color: var(--ring);
    background: var(--sidebar-accent);
  }
  .project-avatar,
  .project-mark {
    color: color-mix(in oklab, var(--foreground) 78%, transparent);
    background: color-mix(in oklab, var(--muted) 84%, var(--sidebar));
    box-shadow: inset 0 1px 0 color-mix(in oklab, var(--foreground) 5%, transparent);
  }
  .project-avatar:hover {
    background: color-mix(in oklab, var(--sidebar-accent) 90%, var(--foreground) 10%);
  }
  .status-dot.stale {
    background: var(--status-warning);
  }
  .status-dot.syncing {
    background: var(--status-info);
  }
  .status-dot.error,
  .collapsed-status.error {
    background: var(--status-error);
  }
  .collapsed-status {
    position: absolute;
    right: -1px;
    bottom: -1px;
    width: 7px;
    height: 7px;
    border: 1.5px solid var(--sidebar);
    border-radius: 999px;
    background: var(--status-success);
  }
  .collapsed-status.stale {
    background: var(--status-warning);
  }
  .collapsed-status.syncing {
    background: var(--status-info);
  }

  .status-dot {
    background: var(--status-success);
  }

  @media (max-width: 1200px) {
    .project-rail:not(.collapsed) {
      flex-basis: 196px;
    }
  }
</style>
