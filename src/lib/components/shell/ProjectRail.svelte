<script lang="ts">
  import { FolderPlus, PanelLeftClose, PanelLeftOpen, SearchX } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { cn } from '$lib/utils.js';
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
  const projectCountLabel = $derived(
    store.projectSearchQuery.trim()
      ? `${filteredProjects.length} of ${store.projects.length}`
      : `${store.projects.length} ${store.projects.length === 1 ? 'project' : 'projects'}`,
  );
</script>

<aside
  class:collapsed={store.projectRailCollapsed}
  class="project-rail flex min-w-0 flex-col text-sidebar-foreground"
  aria-label="Projects"
>
  {#if store.projectRailCollapsed}
    <div class="rail-mode rail-mode-collapsed rail-content">
      <div class="rail-header grid h-14 shrink-0 place-items-center">
        <IconButton
          label="Expand project rail"
          icon={PanelLeftOpen}
          onclick={() => store.toggleProjectRail()}
        />
      </div>
      <div class="grid justify-items-center gap-2 py-3">
        {#each store.projects as project}
          <Tooltip.Root>
            <Tooltip.Trigger>
              {#snippet child({ props })}
                <Button
                  {...props}
                  variant="ghost"
                  size="icon"
                  class={cn(
                    'project-avatar app-focus-ring grid place-items-center rounded-md border border-transparent font-mono text-meta font-semibold transition-[color,background-color,border-color] duration-[var(--dur-micro)] ease-[var(--ease-out)] focus-visible:border-transparent focus-visible:ring-0',
                    project.id === store.selectedProjectId && 'active',
                  )}
                  aria-label={`Open ${project.name}`}
                  onclick={() => store.selectProject(project.id)}
                  >{project.name.slice(0, 2).toUpperCase()}</Button
                >
              {/snippet}
            </Tooltip.Trigger>
            <Tooltip.Content side="right" sideOffset={4}>
              <span class="text-caption">{project.name}</span>
            </Tooltip.Content>
          </Tooltip.Root>
        {/each}
      </div>
    </div>
  {:else}
    <div class="rail-mode rail-mode-expanded rail-content">
      <div class="rail-header flex h-14 shrink-0 items-center justify-between px-2 pl-4">
        <span class="text-body font-semibold">Projects</span>
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

      <div
        class="flex items-center justify-between px-4 pt-1 pb-2 text-caption text-muted-foreground"
      >
        <span>Registered</span>
        <span class="font-mono">{projectCountLabel}</span>
      </div>

      <ScrollArea class="min-h-0 flex-1">
        <div class:empty={filteredProjects.length === 0} class="project-list">
          {#if filteredProjects.length === 0}
            <div class="project-empty app-state-fill" role="status">
              <span class="app-state-icon"><SearchX strokeWidth={1.7} aria-hidden="true" /></span>
              <strong>No matching projects</strong>
              <span>Try a broader filter.</span>
              <Button variant="ghost" size="xs" onclick={() => (store.projectSearchQuery = '')}
                >Clear filter</Button
              >
            </div>
          {:else}
            {#each filteredProjects as project}
              <Button
                variant="ghost"
                class={cn(
                  'project-row app-focus-ring group flex h-auto min-w-0 items-center rounded-md border border-transparent px-2 py-2.5 text-left font-normal transition-[color,background-color,border-color] duration-[var(--dur-micro)] ease-[var(--ease-out)] hover:bg-sidebar-accent focus-visible:border-transparent focus-visible:ring-0',
                  project.id === store.selectedProjectId && 'active',
                )}
                aria-current={project.id === store.selectedProjectId ? 'page' : undefined}
                onclick={() => store.selectProject(project.id)}
              >
                <span class="grid min-w-0 flex-1 gap-0.5"
                  ><strong class="truncate text-body font-medium">{project.name}</strong><small
                    class="truncate font-mono text-meta text-muted-foreground"
                    >{project.branch} · {project.lastScanAt}</small
                  ></span
                >
              </Button>
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
  }

  .rail-mode-collapsed {
    width: 56px;
    min-width: 56px;
    align-items: center;
  }

  :global(.project-row.active) {
    border-color: var(--sidebar-border);
    background: var(--sidebar-accent);
    box-shadow: var(--selected-shadow);
  }
  :global(.project-avatar.active) {
    border-color: var(--ring);
    background: var(--sidebar-accent);
    box-shadow: var(--selected-shadow);
  }
  :global(.project-avatar),
  :global(.project-row) {
    --app-focus-ring-offset: -3px;
  }
  :global(.project-avatar) {
    color: color-mix(in oklab, var(--foreground) 78%, transparent);
    background: color-mix(in oklab, var(--muted) 84%, var(--sidebar));
    box-shadow: var(--control-shadow);
  }
  :global(.project-avatar:hover) {
    background: color-mix(in oklab, var(--sidebar-accent) 90%, var(--foreground) 10%);
  }

  .project-list {
    display: grid;
    height: 100%;
    min-height: 0;
    align-content: start;
    gap: 2px;
    padding: 2px 12px 12px;
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

  .project-empty .app-state-icon {
    margin-bottom: 4px;
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
</style>
