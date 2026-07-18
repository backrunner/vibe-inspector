<script lang="ts">
  import { FolderPlus, PanelLeftClose, PanelLeftOpen, SearchX } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import GlassSearchField from '$components/GlassSearchField.svelte';
  import IconButton from '$components/IconButton.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const filteredProjects = $derived(
    store.projects.filter((project) => {
      const query = store.projectSearchQuery.trim().toLocaleLowerCase();
      if (!query) return true;
      return [project.name, project.branch, project.rootPath].some((value) =>
        value.toLocaleLowerCase().includes(query),
      );
    }),
  );
</script>

<aside
  class:collapsed={store.projectRailCollapsed}
  class="project-rail flex min-w-0 flex-col text-sidebar-foreground"
  aria-label="Projects"
>
  {#if store.projectRailCollapsed}
    <div class="rail-mode rail-mode-collapsed">
      <div class="rail-header grid h-11 shrink-0 place-items-center">
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
            class="project-avatar app-focus-ring grid size-8 place-items-center rounded-md border border-transparent font-mono text-[9px] font-semibold transition-[color,background-color,border-color] duration-[var(--dur-micro)] ease-[var(--ease-out)]"
            aria-label={`Open ${project.name}`}
            title={project.name}
            onclick={() => store.selectProject(project.id)}
            >{project.name.slice(0, 2).toUpperCase()}</button
          >
        {/each}
      </div>
    </div>
  {:else}
    <div class="rail-mode rail-mode-expanded">
      <div class="rail-header flex h-11 shrink-0 items-center justify-between px-2 pl-4">
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

      <div class="project-search mx-3 mt-3 mb-2">
        <GlassSearchField bind:value={store.projectSearchQuery} placeholder="Filter projects" />
      </div>

      <ScrollArea class="min-h-0 flex-1">
        <div class:empty={filteredProjects.length === 0} class="project-list">
          {#if filteredProjects.length === 0}
            <div class="project-empty app-state-fill" role="status">
              <SearchX class="size-4" strokeWidth={1.7} aria-hidden="true" />
              <strong>No matching projects</strong>
              <span>Try a broader filter.</span>
              <Button variant="ghost" size="xs" onclick={() => (store.projectSearchQuery = '')}
                >Clear filter</Button
              >
            </div>
          {:else}
            {#each filteredProjects as project}
              <button
                class:active={project.id === store.selectedProjectId}
                class="project-row app-focus-ring group flex min-w-0 items-center rounded-md border border-transparent px-2 py-2.5 text-left transition-[color,background-color,border-color] duration-[var(--dur-micro)] ease-[var(--ease-out)] hover:bg-sidebar-accent"
                aria-current={project.id === store.selectedProjectId ? 'page' : undefined}
                onclick={() => store.selectProject(project.id)}
              >
                <span class="grid min-w-0 flex-1 gap-0.5"
                  ><strong class="truncate text-[13px] font-medium">{project.name}</strong><small
                    class="truncate font-mono text-[10.5px] text-muted-foreground"
                    >{project.branch} · {project.lastScanAt}</small
                  ></span
                >
              </button>
            {/each}
          {/if}
        </div>
      </ScrollArea>
    </div>
  {/if}
</aside>

<style>
  .project-rail {
    position: relative;
    z-index: 2;
    width: 100%;
    overflow: hidden;
    border: 1px solid var(--panel-edge);
    border-radius: var(--radius-xl);
    background: var(--project-rail-surface);
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

  .rail-mode {
    display: flex;
    width: 216px;
    min-width: 216px;
    height: 100%;
    flex-direction: column;
    animation: rail-content-in var(--dur-rail) var(--ease-out) both;
  }

  .rail-mode-collapsed {
    width: 56px;
    min-width: 56px;
    align-items: center;
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
  .project-row.active {
    border-color: var(--sidebar-border);
    background: var(--sidebar-accent);
    box-shadow: var(--selected-shadow);
  }
  .project-avatar.active {
    border-color: var(--ring);
    background: var(--sidebar-accent);
    box-shadow: var(--selected-shadow);
  }
  .project-avatar,
  .project-row {
    --app-focus-ring-offset: -3px;
  }
  .project-avatar {
    color: color-mix(in oklab, var(--foreground) 78%, transparent);
    background: color-mix(in oklab, var(--muted) 84%, var(--sidebar));
    box-shadow: var(--control-shadow);
  }
  .project-avatar:hover {
    background: color-mix(in oklab, var(--sidebar-accent) 90%, var(--foreground) 10%);
  }

  .project-list {
    display: grid;
    height: 100%;
    min-height: 0;
    align-content: start;
    gap: 2px;
    padding: 0 8px 16px;
  }

  .project-list.empty {
    grid-template-rows: minmax(0, 1fr);
    align-content: stretch;
    padding: 0;
  }

  .project-empty {
    min-height: 0;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 11.5px;
  }

  .project-empty strong {
    color: var(--foreground);
    font-weight: 600;
  }

  .project-empty span {
    max-width: 18ch;
    line-height: 1.45;
  }

  @media (max-width: 1200px) {
    .rail-mode-expanded {
      width: 196px;
      min-width: 196px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .rail-mode {
      animation: none;
    }
  }
</style>
