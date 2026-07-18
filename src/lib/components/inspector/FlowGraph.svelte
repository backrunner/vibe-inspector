<script lang="ts">
  import { Maximize2, ZoomIn, ZoomOut } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import IconButton from '$components/IconButton.svelte';
  import { flowGraphToMermaid } from '$lib/adapters/diagram';
  import type { Flow } from '$domain/entities';
  import MermaidDiagram from './MermaidDiagram.svelte';

  let { flow, theme } = $props<{ flow: Flow; theme: 'dark' | 'light' }>();
  let zoom = $state(1);
  let lastFlowId = $state('');
  let viewport = $state<HTMLDivElement>();
  let viewportWidth = $state(0);
  const definition = $derived(flowGraphToMermaid(flow.graph, 'LR'));
  const diagramWidth = $derived.by(() => {
    if (!viewportWidth) return undefined;
    const readableWidth = Math.max(viewportWidth, flow.graph.nodes.length * 120);
    return Math.round(readableWidth * zoom);
  });

  onMount(() => {
    if (!viewport) return;
    const observer = new ResizeObserver(([entry]) => {
      viewportWidth = entry.contentRect.width;
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  });

  $effect(() => {
    const currentFlowId = flow.id;
    if (lastFlowId && currentFlowId !== lastFlowId) zoom = 1;
    lastFlowId = currentFlowId;
  });
</script>

<section class="graph" aria-label="Business flow">
  <header>
    <div class="graph-title">
      <strong>Business flow</strong>
      <span>{flow.graph.nodes.length} steps</span>
    </div>
    <div class="controls">
      <IconButton
        label="Zoom out"
        icon={ZoomOut}
        onclick={() => (zoom = Math.max(0.8, zoom - 0.1))}
      />
      <span>{Math.round(zoom * 100)}%</span>
      <IconButton
        label="Zoom in"
        icon={ZoomIn}
        onclick={() => (zoom = Math.min(1.3, zoom + 0.1))}
      />
      <IconButton label="Reset diagram view" icon={Maximize2} onclick={() => (zoom = 1)} />
    </div>
  </header>

  <div bind:this={viewport} class="viewport">
    <div class="diagram-scale" style:width={diagramWidth ? `${diagramWidth}px` : '100%'}>
      <MermaidDiagram {definition} {theme} label={`${flow.name} business flow`} />
    </div>
  </div>
</section>

<style>
  .graph {
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
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px 0 13px;
    background: var(--panel-header-surface);
    box-shadow: 0 12px 22px -24px color-mix(in oklab, var(--foreground) 22%, transparent);
  }

  header::after {
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 12px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .graph-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .graph-title strong {
    font-size: 13px;
    font-weight: 600;
  }

  .graph-title span,
  .controls > span {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 1px;
  }

  .controls > span {
    width: 34px;
    text-align: center;
  }

  .viewport {
    min-width: 0;
    min-height: 0;
    overflow: auto;
    background: var(--diagram-surface);
  }

  .diagram-scale {
    height: 100%;
    min-height: 100%;
    margin: 0 auto;
  }
</style>
