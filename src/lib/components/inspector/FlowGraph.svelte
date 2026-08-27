<script lang="ts">
  import { ChevronDown, Maximize2, Network, Waypoints, ZoomIn, ZoomOut } from '@lucide/svelte';
  import { onMount, tick } from 'svelte';
  import IconButton from '$components/IconButton.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import * as SegmentedControl from '$lib/components/ui/segmented-control';
  import {
    clampDiagramZoom,
    diagramZoomFromWheel,
    flowGraphToMermaid,
    relationshipGraph,
    type RelationshipDirection,
  } from '$lib/adapters/diagram';
  import type { Flow, FlowNode } from '$domain/entities';
  import { normalizedWheelDelta } from '$domain/diagram-viewport';
  import MermaidDiagram from './MermaidDiagram.svelte';

  let { flow, theme } = $props<{ flow: Flow; theme: 'dark' | 'light' }>();
  let zoom = $state(1);
  let lastFlowId = $state('');
  let viewport = $state<HTMLElement | null>(null);
  let viewportWidth = $state(0);
  let selectedNodeId = $state<string>();
  let relationshipDirection = $state<RelationshipDirection>('all');
  const hasNodes = $derived(flow.graph.nodes.length > 0);
  const selectedNode = $derived(
    flow.graph.nodes.find((node: FlowNode) => node.id === selectedNodeId),
  );
  const visibleGraph = $derived(
    relationshipGraph(flow.graph, selectedNodeId, relationshipDirection),
  );
  const definition = $derived(flowGraphToMermaid(visibleGraph, 'LR'));
  const diagramWidth = $derived.by(() => {
    if (!viewportWidth) return undefined;
    const readableWidth = Math.max(viewportWidth, visibleGraph.nodes.length * 120);
    return Math.round(readableWidth * zoom);
  });

  onMount(() => {
    if (!viewport) return;
    const element = viewport;
    const observer = new ResizeObserver(([entry]) => {
      viewportWidth = entry.contentRect.width;
    });
    observer.observe(element);
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

  function handleWheel(event: WheelEvent) {
    if (!event.ctrlKey && (event.shiftKey || Math.abs(event.deltaX) > Math.abs(event.deltaY)))
      return;
    event.preventDefault();
    void setZoom(
      diagramZoomFromWheel(
        zoom,
        normalizedWheelDelta(event.deltaY, event.deltaMode, viewport?.clientHeight ?? 1),
        event.ctrlKey,
      ),
      event.clientX,
    );
  }

  $effect(() => {
    const currentFlowId = flow.id;
    if (lastFlowId && currentFlowId !== lastFlowId) {
      zoom = 1;
      selectedNodeId = undefined;
      relationshipDirection = 'all';
    }
    lastFlowId = currentFlowId;
  });

  $effect(() => {
    if (selectedNodeId && !selectedNode) selectedNodeId = undefined;
  });
</script>

<section class="graph" aria-label="Business flow">
  <header>
    <div class="graph-title">
      <strong>Business flow</strong>
      <span>{flow.graph.nodes.length} steps</span>
    </div>
    {#if hasNodes}
      <div class="controls">
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Button {...props} variant="ghost" size="xs" class="relationship-trigger">
                <Network class="size-3.5" strokeWidth={1.7} aria-hidden="true" />
                <span>{selectedNode?.label ?? 'Relationships'}</span>
                <ChevronDown class="size-3" strokeWidth={1.8} aria-hidden="true" />
              </Button>
            {/snippet}
          </DropdownMenu.Trigger>
          <DropdownMenu.Content align="end" class="node-menu">
            <DropdownMenu.Item
              disabled={!selectedNodeId}
              onclick={() => (selectedNodeId = undefined)}>All steps</DropdownMenu.Item
            >
            <DropdownMenu.Separator />
            {#each flow.graph.nodes as node (node.id)}
              <DropdownMenu.Item
                disabled={node.id === selectedNodeId}
                onclick={() => (selectedNodeId = node.id)}>{node.label}</DropdownMenu.Item
              >
            {/each}
          </DropdownMenu.Content>
        </DropdownMenu.Root>
        {#if selectedNodeId}
          <SegmentedControl.Root aria-label="Relationship direction">
            {#each ['incoming', 'all', 'outgoing'] as direction}
              <SegmentedControl.Item
                selected={relationshipDirection === direction}
                onclick={() => (relationshipDirection = direction as RelationshipDirection)}
              >
                {direction === 'incoming' ? 'In' : direction === 'outgoing' ? 'Out' : 'All'}
              </SegmentedControl.Item>
            {/each}
          </SegmentedControl.Root>
        {/if}
        <IconButton label="Zoom out" icon={ZoomOut} onclick={() => void setZoom(zoom - 0.1)} />
        <span>{Math.round(zoom * 100)}%</span>
        <IconButton label="Zoom in" icon={ZoomIn} onclick={() => void setZoom(zoom + 0.1)} />
        <IconButton label="Reset diagram view" icon={Maximize2} onclick={() => void setZoom(1)} />
      </div>
    {/if}
  </header>

  <ScrollArea
    bind:viewportRef={viewport}
    class="viewport"
    orientation="both"
    viewportProps={{
      tabindex: 0,
      role: 'region',
      'aria-label': `${flow.name} business flow canvas`,
    }}
  >
    {#if hasNodes}
      <div class="diagram-scale" style:width={diagramWidth ? `${diagramWidth}px` : '100%'}>
        <MermaidDiagram {definition} {theme} label={`${flow.name} business flow`} />
      </div>
    {:else}
      <div class="graph-empty app-state-fill" role="status">
        <Waypoints class="size-4" strokeWidth={1.7} aria-hidden="true" />
        <strong>Business flow missing</strong>
        <span>Ask the agent to add a complete flow graph, then run inspector-sync.</span>
      </div>
    {/if}
  </ScrollArea>
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

  :global(.relationship-trigger) {
    max-width: 180px;
  }

  :global(.node-menu) {
    max-height: min(320px, 60vh);
    overflow-y: auto;
  }

  :global(.relationship-trigger > span) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.controls [data-slot='segmented-control']) {
    margin-right: 3px;
    padding: 1px;
  }

  :global(.controls [data-segmented-control-item]) {
    min-width: 27px;
    height: 22px;
    padding-inline: 5px;
    font-size: 10px;
  }

  .controls > span {
    width: 34px;
    text-align: center;
  }

  .graph-empty {
    width: 100%;
    height: 100%;
    min-height: 0;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 11.5px;
  }

  .graph-empty > strong {
    color: var(--foreground);
    font-weight: 600;
  }

  .graph-empty > span {
    max-width: 24ch;
    line-height: 1.45;
  }

  :global(.viewport) {
    min-width: 0;
    min-height: 0;
    background: var(--diagram-surface);
  }

  :global(.viewport [data-slot='scroll-area-viewport']) {
    overscroll-behavior: contain;
  }

  .diagram-scale {
    height: 100%;
    min-height: 100%;
    margin: 0 auto;
  }
</style>
