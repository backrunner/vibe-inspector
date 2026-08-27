<script lang="ts">
  import { GripVertical } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import InspectorWorkspace from '$components/inspector/InspectorWorkspace.svelte';
  import ModuleRail from '$components/modules/ModuleRail.svelte';
  import {
    clampModuleRailWidth,
    moduleRailMaxWidth,
    moduleRailMinWidth,
  } from '$domain/workspace-layout';
  import ProjectRail from './ProjectRail.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const moduleWidthStorageKey = 'vibe-inspector:module-rail-width';
  let paneGroup = $state<HTMLDivElement>();
  let paneGroupWidth = $state(0);
  let moduleRailWidth = $state(280);
  let resizing = $state(false);
  let dragStartX = 0;
  let dragStartWidth = 0;
  const moduleRailEffectiveMax = $derived(
    paneGroupWidth
      ? clampModuleRailWidth(moduleRailMaxWidth, paneGroupWidth)
      : moduleRailMaxWidth,
  );

  function constrainedWidth(width: number): number {
    return paneGroup ? clampModuleRailWidth(width, paneGroup.clientWidth) : width;
  }

  function beginResize(event: PointerEvent) {
    if (event.button !== 0 || store.moduleRailCollapsed) return;
    event.preventDefault();
    const target = event.currentTarget as HTMLDivElement;
    target.setPointerCapture(event.pointerId);
    resizing = true;
    dragStartX = event.clientX;
    dragStartWidth = moduleRailWidth;
  }

  function resizePane(event: PointerEvent) {
    if (!resizing) return;
    moduleRailWidth = constrainedWidth(dragStartWidth + event.clientX - dragStartX);
  }

  function finishResize(event: PointerEvent) {
    if (!resizing) return;
    resizing = false;
    const target = event.currentTarget as HTMLDivElement;
    if (target.hasPointerCapture(event.pointerId)) {
      target.releasePointerCapture(event.pointerId);
    }
    localStorage.setItem(moduleWidthStorageKey, String(moduleRailWidth));
  }

  function resizeWithKeyboard(event: KeyboardEvent) {
    if (store.moduleRailCollapsed) return;
    let nextWidth = moduleRailWidth;
    if (event.key === 'ArrowLeft') nextWidth -= event.shiftKey ? 32 : 16;
    else if (event.key === 'ArrowRight') nextWidth += event.shiftKey ? 32 : 16;
    else if (event.key === 'Home') nextWidth = moduleRailMinWidth;
    else if (event.key === 'End') nextWidth = moduleRailMaxWidth;
    else return;

    event.preventDefault();
    moduleRailWidth = constrainedWidth(nextWidth);
    localStorage.setItem(moduleWidthStorageKey, String(moduleRailWidth));
  }

  onMount(() => {
    if (!paneGroup) return;
    const element = paneGroup;
    paneGroupWidth = element.clientWidth;
    const storedWidth = Number.parseInt(localStorage.getItem(moduleWidthStorageKey) ?? '', 10);
    if (Number.isFinite(storedWidth)) moduleRailWidth = constrainedWidth(storedWidth);
    else moduleRailWidth = constrainedWidth(moduleRailWidth);

    const observer = new ResizeObserver(() => {
      paneGroupWidth = element.clientWidth;
      moduleRailWidth = constrainedWidth(moduleRailWidth);
    });
    observer.observe(element);
    return () => observer.disconnect();
  });
</script>

<div
  class:project-collapsed={store.projectRailCollapsed}
  class:module-collapsed={store.moduleRailCollapsed}
  class="registered-workspace"
>
  <ProjectRail />
  <div
    bind:this={paneGroup}
    class:module-collapsed={store.moduleRailCollapsed}
    class:resizing
    class="main-panes"
    style={`--module-rail-width: ${moduleRailWidth}px`}
  >
    <ModuleRail />
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="pane-resizer"
      role="separator"
      aria-label="Resize Architecture and Inspector panes"
      aria-orientation="vertical"
      aria-valuemin={moduleRailMinWidth}
      aria-valuemax={moduleRailEffectiveMax}
      aria-valuenow={moduleRailWidth}
      aria-valuetext={`Architecture pane ${moduleRailWidth} pixels wide`}
      tabindex="0"
      title="Drag to resize Architecture"
      onpointerdown={beginResize}
      onpointermove={resizePane}
      onpointerup={finishResize}
      onpointercancel={finishResize}
      onkeydown={resizeWithKeyboard}
    >
      <GripVertical strokeWidth={1.7} aria-hidden="true" />
    </div>
    <InspectorWorkspace />
  </div>
</div>

<style>
  .registered-workspace {
    display: grid;
    box-sizing: border-box;
    min-width: 0;
    min-height: 0;
    flex: 1;
    grid-template-columns: 216px minmax(0, 1fr);
    gap: 8px;
    overflow: hidden;
    padding: 0 8px 8px;
    transition: grid-template-columns var(--dur-rail) var(--ease-in-out);
  }

  .registered-workspace.project-collapsed {
    grid-template-columns: 56px minmax(0, 1fr);
  }

  .main-panes {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: var(--module-rail-width) 8px minmax(560px, 1fr);
    overflow: hidden;
    transition: grid-template-columns var(--dur-rail) var(--ease-in-out);
  }

  .main-panes.module-collapsed {
    grid-template-columns: 48px 0 minmax(560px, 1fr);
  }

  .main-panes.resizing {
    transition: none;
  }

  .pane-resizer {
    position: relative;
    z-index: 3;
    display: grid;
    width: 8px;
    min-width: 8px;
    height: 100%;
    place-items: center;
    border: 0;
    color: var(--muted-foreground);
    background: transparent;
    cursor: col-resize;
    touch-action: none;
  }

  .pane-resizer::before {
    position: absolute;
    top: 12px;
    bottom: 12px;
    left: 3px;
    width: 2px;
    border-radius: 2px;
    background: var(--panel-divider);
    content: '';
    transition:
      background-color var(--dur-micro) var(--ease-out),
      box-shadow var(--dur-micro) var(--ease-out);
  }

  .pane-resizer::after {
    position: absolute;
    inset: 0 -8px;
    content: '';
  }

  .pane-resizer :global(svg) {
    position: relative;
    z-index: 1;
    width: 12px;
    height: 18px;
    opacity: 0;
    background: var(--workspace);
    transition: opacity var(--dur-micro) var(--ease-out);
  }

  .pane-resizer:hover::before,
  .pane-resizer:focus-visible::before,
  .main-panes.resizing .pane-resizer::before {
    background: var(--ring);
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--ring) 14%, transparent);
  }

  .pane-resizer:hover :global(svg),
  .pane-resizer:focus-visible :global(svg),
  .main-panes.resizing .pane-resizer :global(svg) {
    opacity: 1;
  }

  .pane-resizer:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .main-panes.module-collapsed .pane-resizer {
    display: none;
  }

  @media (max-width: 1200px) {
    .registered-workspace {
      grid-template-columns: 196px minmax(0, 1fr);
    }

    .registered-workspace.project-collapsed {
      grid-template-columns: 56px minmax(0, 1fr);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .registered-workspace,
    .main-panes {
      transition: none;
    }
  }
</style>
