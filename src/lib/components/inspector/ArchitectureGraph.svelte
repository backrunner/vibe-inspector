<script lang="ts">
  import { Maximize2, Network, ZoomIn, ZoomOut } from '@lucide/svelte';
  import { tick } from 'svelte';
  import IconButton from '$components/IconButton.svelte';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { architectureGraphToMermaid, moduleArchitectureGraph } from '$lib/adapters/architecture';
  import { clampDiagramZoom, diagramZoomFromWheel } from '$lib/adapters/diagram';
  import { normalizedWheelDelta } from '$domain/diagram-viewport';
  import type { Module } from '$domain/entities';
  import MermaidDiagram from './MermaidDiagram.svelte';

  let { modules, selectedModuleId, theme } = $props<{
    modules: Module[];
    selectedModuleId?: string;
    theme: 'dark' | 'light';
  }>();
  let zoom = $state(1);
  let lastModuleId = $state<string>();
  let viewport = $state<HTMLElement | null>(null);
  let viewportWidth = $state(0);
  const graph = $derived(moduleArchitectureGraph(modules, selectedModuleId));
  const definition = $derived(architectureGraphToMermaid(graph));
  const hasNodes = $derived(graph.nodes.length > 0);
  const diagramWidth = $derived.by(() => {
    if (!viewportWidth) return undefined;
    return Math.round(Math.max(viewportWidth, graph.nodes.length * 150) * zoom);
  });

  $effect(() => {
    if (!viewport) return;
    const element = viewport;
    const observer = new ResizeObserver(([entry]) => (viewportWidth = entry.contentRect.width));
    observer.observe(element);
    const handleWheel = (event: WheelEvent) => {
      if (!event.ctrlKey && (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY))) return;
      event.preventDefault();
      void setZoom(
        diagramZoomFromWheel(
          zoom,
          normalizedWheelDelta(event.deltaY, event.deltaMode, viewport?.clientHeight ?? 1),
          event.ctrlKey,
        ),
        event.clientX,
      );
    };
    element.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      observer.disconnect();
      element.removeEventListener('wheel', handleWheel);
    };
  });

  async function setZoom(nextZoom: number, clientX?: number) {
    if (!viewport) return;
    const previousZoom = zoom;
    const clampedZoom = clampDiagramZoom(nextZoom);
    if (clampedZoom === previousZoom) return;

    const bounds = viewport.getBoundingClientRect();
    const pointerOffset = (clientX ?? bounds.left + bounds.width / 2) - bounds.left;
    const contentOffset = viewport.scrollLeft + pointerOffset;
    zoom = clampedZoom;
    await tick();
    viewport.scrollLeft = contentOffset * (clampedZoom / previousZoom) - pointerOffset;
  }

  $effect(() => {
    const currentModuleId = selectedModuleId;
    if (lastModuleId !== undefined && currentModuleId !== lastModuleId) zoom = 1;
    lastModuleId = currentModuleId;
  });
</script>

<section class="architecture-graph" aria-label="Module architecture">
  <header>
    <div class="graph-title">
      <strong>Module architecture</strong>
      <span>{graph.nodes.length} modules · containment only</span>
    </div>
    {#if hasNodes}
      <div class="controls">
        <IconButton label="Zoom out" icon={ZoomOut} onclick={() => void setZoom(zoom - 0.1)} />
        <span>{Math.round(zoom * 100)}%</span>
        <IconButton label="Zoom in" icon={ZoomIn} onclick={() => void setZoom(zoom + 0.1)} />
        <IconButton label="Reset architecture view" icon={Maximize2} onclick={() => void setZoom(1)} />
      </div>
    {/if}
  </header>
  <ScrollArea
    bind:viewportRef={viewport}
    class="viewport"
    orientation="both"
    viewportProps={{ tabindex: 0, role: 'region', 'aria-label': 'Module architecture canvas' }}
  >
    {#if hasNodes}
      <div class="diagram-scale" style:width={diagramWidth ? `${diagramWidth}px` : '100%'}>
        <MermaidDiagram
          {definition}
          {theme}
          label="Module architecture diagram"
          errorTitle="Module architecture could not be rendered"
          errorHint="Ask the agent to correct the module graph, then run inspector-sync."
        />
      </div>
    {:else}
      <div class="graph-empty app-state-fill" role="status">
        <Network class="size-4" strokeWidth={1.7} aria-hidden="true" />
        <strong>Architecture unavailable</strong>
        <span>Register a project and run inspector-init to build its module map.</span>
      </div>
    {/if}
  </ScrollArea>
</section>

<style>
  .architecture-graph {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-rows: 42px minmax(0, 1fr);
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: var(--panel-shadow);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px 0 13px;
    background: var(--panel-header-surface);
  }

  .graph-title { display: flex; align-items: baseline; gap: 8px; }
  .graph-title strong { font-size: 13px; font-weight: 600; }
  .graph-title span, .controls > span {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }
  .controls { display: flex; align-items: center; gap: 1px; }
  .controls > span { width: 34px; text-align: center; }
  .diagram-scale { height: 100%; min-height: 100%; margin: 0 auto; }
  .graph-empty { width: 100%; height: 100%; gap: 8px; color: var(--muted-foreground); font-size: 11.5px; }
  .graph-empty > strong { color: var(--foreground); font-weight: 600; }
  .graph-empty > span { max-width: 28ch; line-height: 1.45; }
  :global(.viewport) { min-width: 0; min-height: 0; background: var(--diagram-surface); }
  :global(.viewport [data-slot='scroll-area-viewport']) { overscroll-behavior: contain; }
</style>
