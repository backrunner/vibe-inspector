<script lang="ts">
  import { fade } from 'svelte/transition';
  import { Check, ChevronRight, FolderGit2, Lightbulb, TriangleAlert } from '@lucide/svelte';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import * as SegmentedControl from '$lib/components/ui/segmented-control';
  import CodeViewer from '$components/code/CodeViewer.svelte';
  import SourceFileTree from '$components/code/SourceFileTree.svelte';
  import SourceViewerSkeleton from '$components/code/SourceViewerSkeleton.svelte';
  import FlowGraph from './FlowGraph.svelte';
  import InspectorSkeleton from './InspectorSkeleton.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const flow = $derived(store.selectedFlow);
  const sourceRef = $derived(store.selectedSourceRef);
  let overviewPanel = $state<'flow' | 'rationale'>('flow');

  // Keyed only on the top-level view state, never on the selected flow id, so
  // switching flows updates props in place instead of remounting FlowGraph/CodeViewer.
  const viewStateKey = $derived(
    flow
      ? 'content'
      : store.selectedProject
        ? store.inspectorStatus === 'loading'
          ? 'loading'
          : store.inspectorStatus === 'error'
            ? 'error'
            : 'no-data'
        : 'no-selection',
  );

  function formatSource(source: string): string {
    return source
      .split('-')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }

  function formatGeneratedAt(value: string): string {
    const date = new Date(value);
    if (Number.isNaN(date.valueOf())) return value;
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(date);
  }
</script>

<main
  class="inspector-workspace grid min-w-0 flex-1 overflow-hidden"
  aria-label="Inspector workspace"
>
  {#key viewStateKey}
    {#if flow}
      <div class="workspace-content" transition:fade={{ duration: 150 }}>
        <section class="flow-heading">
          <div class="min-w-0">
            <div class="flow-context">
              <span class="truncate">{store.selectedProject?.name ?? 'No project'}</span>
              <ChevronRight class="size-3 shrink-0" />
              <strong class="truncate">{store.selectedModule?.name ?? 'No module'}</strong>
            </div>
            <h1>{flow.name}</h1>
            <p>{flow.summary}</p>
          </div>
        </section>

        <section class="overview-section" data-panel={overviewPanel} aria-label="Flow overview">
          <SegmentedControl.Root class="overview-toggle" aria-label="Flow overview panel">
            <SegmentedControl.Item
              class="min-w-[72px]"
              selected={overviewPanel === 'flow'}
              onclick={() => (overviewPanel = 'flow')}>Flow</SegmentedControl.Item
            >
            <SegmentedControl.Item
              class="min-w-[72px]"
              selected={overviewPanel === 'rationale'}
              onclick={() => (overviewPanel = 'rationale')}>Rationale</SegmentedControl.Item
            >
          </SegmentedControl.Root>

          <div class="flow-panel">
            <FlowGraph {flow} theme={store.theme} />
          </div>

          <aside class="rationale" aria-label="Design rationale">
            <header>
              <span class="rationale-icon"><Lightbulb class="size-3.5" strokeWidth={1.8} /></span>
              <div>
                <strong>Design rationale</strong>
                <span
                  >{flow.explanation
                    ? `${Math.round(flow.explanation.confidence * 100)}% confidence`
                    : 'Not documented'}</span
                >
              </div>
            </header>
            <ScrollArea
              class="min-h-0 flex-1"
              viewportProps={{
                tabindex: 0,
                role: 'region',
                'aria-label': 'Design rationale details',
              }}
            >
              {#if flow.explanation}
                <div class="rationale-content">
                  <h2>{flow.explanation.headline}</h2>
                  <p>{flow.explanation.body}</p>
                  {#if flow.explanation.decisions.length > 0}
                    <div class="decisions">
                      <strong>Key decisions</strong>
                      {#each flow.explanation.decisions as decision}
                        <div>
                          <Check class="decision-check" strokeWidth={2} /><span>{decision}</span>
                        </div>
                      {/each}
                    </div>
                  {/if}
                  <dl class="provenance">
                    <div>
                      <dt>Source</dt>
                      <dd>{formatSource(flow.explanation.provenance.source)}</dd>
                    </div>
                    {#if flow.explanation.provenance.model}
                      <div>
                        <dt>Model</dt>
                        <dd>{flow.explanation.provenance.model}</dd>
                      </div>
                    {/if}
                    <div>
                      <dt>Generated</dt>
                      <dd>{formatGeneratedAt(flow.explanation.provenance.generatedAt)}</dd>
                    </div>
                    <div>
                      <dt>Evidence</dt>
                      <dd>{flow.explanation.provenance.evidenceIds.length} references</dd>
                    </div>
                  </dl>
                </div>
              {:else}
                <div class="rationale-empty app-state-fill">
                  The agent has not recorded a design rationale for this flow yet.
                </div>
              {/if}
            </ScrollArea>
          </aside>
        </section>

        <section class="source-section" aria-label="Source evidence">
          <header class="source-heading">
            <div>
              <strong>Source evidence</strong>
              <span>Read-only files associated with this flow</span>
            </div>
            <span class="source-count"
              >{flow.sourceRefs.length}
              {flow.sourceRefs.length === 1 ? 'reference' : 'references'}{store.relatedSourceRefs
                .length
                ? ` · ${store.relatedSourceRefs.length} related`
                : ''}</span
            >
          </header>

          <div class="source-grid">
            <SourceFileTree
              sourceRefs={store.visibleSourceRefs}
              selectedRefId={store.selectedSourceRefId}
              onselect={(refId) => store.selectSourceRef(refId)}
            />

            <div class="min-w-0 min-h-0">
              {#if sourceRef && store.sourceStatus === 'ready'}
                <CodeViewer
                  file={sourceRef}
                  code={store.sourceCode}
                  codeStartLine={store.sourceStartLine}
                  readStatus={store.sourceReadStatus}
                  theme={store.theme}
                  embedded
                />
              {:else if sourceRef && store.sourceStatus === 'loading'}
                <SourceViewerSkeleton />
              {:else if sourceRef && store.sourceStatus === 'error'}
                <div class="source-state source-error app-state-fill">
                  <TriangleAlert aria-hidden="true" />
                  <div><strong>Source unavailable</strong><span>{store.sourceError}</span></div>
                </div>
              {:else}
                <div class="source-state app-state-fill">No source selected</div>
              {/if}
            </div>
          </div>
        </section>
      </div>
    {:else if store.selectedProject && store.inspectorStatus === 'loading'}
      <div class="skeleton-pane" transition:fade={{ duration: 150 }}>
        <InspectorSkeleton />
      </div>
    {:else if store.selectedProject && store.inspectorStatus === 'error'}
      <div class="empty-inspector app-state-fill" transition:fade={{ duration: 150 }}>
        <span><TriangleAlert strokeWidth={1.7} /></span>
        <strong>{store.selectedProject.name}</strong>
        <h1>Inspector data unavailable</h1>
        <p>{store.inspectorError}</p>
      </div>
    {:else if store.selectedProject}
      <div class="empty-inspector app-state-fill" transition:fade={{ duration: 150 }}>
        <span><FolderGit2 strokeWidth={1.7} /></span>
        <strong>{store.selectedProject.name}</strong>
        <h1>No inspector data yet</h1>
        <p>
          Run <code>$inspector-init</code> from Codex for this project, then return here to inspect the
          result.
        </p>
      </div>
    {:else}
      <div
        class="empty-inspector app-state-fill empty-inspector-quiet"
        transition:fade={{ duration: 150 }}
      >
        <span><FolderGit2 strokeWidth={1.7} /></span>
        <h1>Select a project</h1>
        <p>Choose a registered project from the Projects rail to inspect its architecture.</p>
      </div>
    {/if}
  {/key}
</main>

<style>
  .inspector-workspace {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    border: 1px solid var(--panel-edge);
    border-radius: var(--radius-xl);
    background: var(--workspace-panel-surface);
    box-shadow: var(--panel-shadow);
  }

  /* Outgoing and incoming state panes share one grid cell, so the cross-fade
     overlaps instead of reflowing the workspace. */
  .inspector-workspace > * {
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
  }

  .skeleton-pane {
    display: flex;
    min-height: 0;
    flex-direction: column;
  }

  .workspace-content {
    display: grid;
    min-height: 0;
    flex: 1;
    grid-template-rows: auto minmax(190px, 0.9fr) minmax(262px, 1.15fr);
    gap: 12px;
    overflow: hidden;
    padding: 16px 20px;
  }

  .empty-inspector {
    flex: 1;
    gap: 0;
  }

  .empty-inspector > * {
    max-width: 460px;
  }

  .empty-inspector > span {
    display: grid;
    width: 36px;
    height: 36px;
    margin-bottom: 14px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    background: var(--muted);
    box-shadow: var(--control-shadow);
  }

  .empty-inspector > span :global(svg) {
    width: 18px;
  }

  .empty-inspector > strong {
    color: var(--muted-foreground);
    font-size: 12px;
    font-weight: 550;
  }

  .empty-inspector h1 {
    margin: 7px 0 6px;
    font-size: 20px;
    font-weight: 640;
    letter-spacing: 0;
  }

  .empty-inspector p {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.55;
  }

  .empty-inspector code {
    color: var(--foreground);
    font-family: var(--app-font-mono);
    font-size: 12px;
  }

  .empty-inspector-quiet > span {
    color: var(--muted-foreground);
  }

  .flow-heading {
    display: flex;
    min-width: 0;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
  }

  .flow-context {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
    margin-bottom: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1;
  }

  .flow-context strong {
    color: color-mix(in oklab, var(--foreground) 82%, transparent);
    font-weight: 560;
  }

  .flow-heading h1 {
    margin: 0 0 6px;
    font-size: 24px;
    line-height: 1.1;
    font-weight: 650;
    letter-spacing: 0;
  }

  .flow-heading p {
    max-width: 72ch;
    margin: 0;
    color: var(--muted-foreground);
    font-size: 13.5px;
    line-height: 1.55;
  }

  .overview-section {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: minmax(0, 1fr) 400px;
    gap: 12px;
  }

  :global([data-slot='segmented-control'].overview-toggle) {
    display: none;
  }

  .flow-panel {
    min-width: 0;
    min-height: 0;
  }

  .flow-panel :global(.graph) {
    height: 100%;
  }

  .rationale {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: var(--panel-shadow);
  }

  .rationale > header {
    position: relative;
    display: flex;
    min-height: 46px;
    flex: 0 0 46px;
    align-items: center;
    gap: 9px;
    padding: 0 12px;
    background: var(--panel-header-surface);
    box-shadow: 0 12px 22px -24px color-mix(in oklab, var(--foreground) 22%, transparent);
  }

  .rationale > header::after {
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 12px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .rationale-icon {
    display: grid;
    width: 28px;
    height: 28px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    background: var(--muted);
    box-shadow: var(--control-shadow);
  }

  .rationale > header > div {
    display: grid;
    gap: 1px;
  }

  .rationale > header strong {
    font-size: 13px;
    font-weight: 600;
  }

  .rationale > header span,
  .source-count {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .rationale-content {
    width: 100%;
    max-width: 58ch;
    margin: 0 auto;
    padding: 16px 16px 20px;
    box-sizing: border-box;
  }

  .rationale-empty {
    min-height: 100%;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.6;
  }

  .rationale-content h2 {
    margin: 0 0 8px;
    font-size: 18px;
    line-height: 1.35;
    font-weight: 620;
    letter-spacing: 0;
  }

  .rationale-content > p {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.62;
  }

  .decisions {
    display: grid;
    gap: 10px;
    margin-top: 18px;
  }

  .decisions > strong {
    color: var(--muted-foreground);
    font-size: 11px;
    font-weight: 560;
  }

  .decisions > div {
    display: grid;
    grid-template-columns: 14px 1fr;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.5;
  }

  .provenance {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px 14px;
    margin: 18px 0 0;
    padding-top: 14px;
    box-shadow: 0 -12px 22px -24px color-mix(in oklab, var(--foreground) 22%, transparent);
  }

  .provenance > div {
    display: grid;
    min-width: 0;
    gap: 3px;
  }

  .provenance dt,
  .provenance dd {
    margin: 0;
  }

  .provenance dt {
    color: var(--muted-foreground);
    font-size: 10.5px;
  }

  .provenance dd {
    overflow: hidden;
    color: color-mix(in oklab, var(--foreground) 88%, transparent);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.decision-check) {
    width: 12px;
    margin-top: 1px;
    color: var(--status-success);
  }

  .source-section {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-rows: 34px minmax(0, 1fr);
    gap: 8px;
  }

  .source-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .source-heading > div {
    display: flex;
    min-width: 0;
    align-items: baseline;
    gap: 9px;
  }

  .source-heading strong {
    font-size: 13px;
    font-weight: 600;
  }

  .source-heading > div > span {
    overflow: hidden;
    color: var(--muted-foreground);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-grid {
    display: grid;
    min-width: 0;
    min-height: 0;
    grid-template-columns: 220px minmax(0, 1fr);
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: var(--panel-shadow);
  }

  .source-state {
    width: 100%;
    height: 100%;
    min-height: 0;
    gap: 8px;
    border: 0;
    border-radius: 0;
    color: var(--muted-foreground);
    background: var(--card);
    box-shadow: none;
    font-size: 12px;
    padding: 0;
  }

  .source-state :global(svg) {
    width: 15px;
    height: 15px;
  }

  .source-error {
    color: var(--destructive);
  }

  .source-error > div {
    display: grid;
    gap: 2px;
    text-align: center;
  }

  .source-error strong {
    color: var(--foreground);
    font-size: 12px;
    font-weight: 600;
  }

  .source-error span {
    max-width: 46ch;
    font-size: 11px;
  }

  @media (max-width: 1200px) {
    .workspace-content {
      grid-template-rows: auto minmax(220px, 0.95fr) minmax(250px, 1.1fr);
      padding: 14px;
    }

    .overview-section {
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: 28px minmax(0, 1fr);
      gap: 8px;
    }

    :global([data-slot='segmented-control'].overview-toggle) {
      display: inline-flex;
      justify-self: end;
    }

    .flow-panel,
    .rationale {
      grid-row: 2;
      grid-column: 1;
    }

    .overview-section[data-panel='flow'] .rationale,
    .overview-section[data-panel='rationale'] .flow-panel {
      display: none;
    }

    .source-grid {
      grid-template-columns: 220px minmax(0, 1fr);
    }
  }
</style>
