<script lang="ts">
  import { Check, Clock3, TriangleAlert } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as SegmentedControl from '$lib/components/ui/segmented-control';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { formatActivityTime, formatRelativeActivity } from '$lib/adapters/activity';
  import { flowGraphToMermaid } from '$lib/adapters/diagram';
  import { flowVersionSnapshot, moduleVersionSnapshot } from '$lib/adapters/versions';
  import MermaidDiagram from '$components/inspector/MermaidDiagram.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  const flowSnapshot = $derived(
    store.selectedVersion ? flowVersionSnapshot(store.selectedVersion) : null,
  );
  const moduleSnapshot = $derived(
    store.selectedVersion ? moduleVersionSnapshot(store.selectedVersion) : null,
  );
  const definition = $derived(
    flowSnapshot && flowSnapshot.graph.nodes.length > 0
      ? flowGraphToMermaid(
          {
            nodes: flowSnapshot.graph.nodes.map((node) => ({ ...node, x: 0, y: 0 })),
            edges: flowSnapshot.graph.edges,
          },
          'LR',
        )
      : '',
  );

  function formatSource(source: string): string {
    return source
      .split('-')
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(' ');
  }
</script>

<Dialog.Root bind:open={store.historyOpen}>
  <Dialog.Content class="history-dialog gap-0 overflow-hidden p-0 sm:max-w-[760px]">
    <Dialog.Header class="history-header border-b px-5 py-4 text-left">
      <div class="min-w-0">
        <Dialog.Title class="text-heading">Version history</Dialog.Title>
        <Dialog.Description class="text-rail">
          {store.historyEntityName || 'No entity selected'}
        </Dialog.Description>
      </div>
      <SegmentedControl.Root aria-label="Version history entity">
        <SegmentedControl.Item
          class="min-w-[68px]"
          selected={store.historyEntityType === 'module'}
          onclick={() => void store.setHistoryEntityType('module')}>Module</SegmentedControl.Item
        >
        <SegmentedControl.Item
          class="min-w-[68px]"
          selected={store.historyEntityType === 'flow'}
          disabled={!store.selectedFlow}
          onclick={() => void store.setHistoryEntityType('flow')}>Flow</SegmentedControl.Item
        >
      </SegmentedControl.Root>
    </Dialog.Header>

    {#if store.historyStatus === 'loading'}
      <div class="history-loading" aria-label="Loading version history" aria-busy="true">
        {#each [1, 2, 3, 4] as row}
          <div>
            <Skeleton class="h-3 w-14" /><Skeleton class="h-3 w-32" /><Skeleton
              class="h-2.5 w-20"
            />
          </div>
        {/each}
      </div>
    {:else if store.historyStatus === 'error'}
      <div class="history-state error app-state-fill">
        <TriangleAlert /><span>{store.historyError}</span>
      </div>
    {:else if store.historyVersions.length === 0}
      <div class="history-state app-state-fill">
        <Clock3 /><span>No recorded versions for this {store.historyEntityType} yet.</span>
      </div>
    {:else}
      <div class="history-body">
        <div class="history-list-pane">
          <ScrollArea class="h-full min-h-0">
            <div class="history-list" role="listbox" aria-label="Recorded versions">
              {#each store.historyVersions as version (version.id)}
                <button
                  type="button"
                  class="history-row"
                  class:selected={version.id === store.selectedVersionId}
                  role="option"
                  aria-selected={version.id === store.selectedVersionId}
                  onclick={() => void store.selectVersion(version.id)}
                >
                  <strong>r{version.revision}</strong>
                  <span>{version.actor} · {formatSource(version.source)}</span>
                  <time title={formatActivityTime(version.createdAt)}
                    >{formatRelativeActivity(version.createdAt)}</time
                  >
                </button>
              {/each}
            </div>
          </ScrollArea>
        </div>

        <div class="history-detail">
          {#if store.versionDetailStatus === 'loading'}
            <div class="detail-loading" aria-label="Loading version" aria-busy="true">
              <Skeleton class="h-4 w-48" />
              <Skeleton class="h-3 w-full" />
              <Skeleton class="h-3 w-2/3" />
              <Skeleton class="h-40 w-full" />
            </div>
          {:else if store.versionDetailStatus === 'error'}
            <div class="history-state error app-state-fill">
              <TriangleAlert /><span>{store.versionDetailError}</span>
            </div>
          {:else if flowSnapshot}
            <ScrollArea class="min-h-0 flex-1">
              <div class="detail-content">
                <header class="detail-heading">
                  <strong>{flowSnapshot.name}</strong>
                  <span
                    >{flowSnapshot.status} · r{store.selectedVersion?.revision} · {formatActivityTime(
                      store.selectedVersion?.createdAt ?? '',
                    )}</span
                  >
                </header>
                {#if flowSnapshot.summary}<p class="detail-summary">{flowSnapshot.summary}</p>{/if}
                {#if definition}
                  <div class="detail-diagram">
                    <MermaidDiagram
                      {definition}
                      theme={store.theme}
                      label={`${flowSnapshot.name} flow at revision ${store.selectedVersion?.revision}`}
                    />
                  </div>
                {/if}
                {#if flowSnapshot.explanation}
                  <div class="detail-section">
                    <h2>{flowSnapshot.explanation.headline}</h2>
                    <p>{flowSnapshot.explanation.body}</p>
                    {#if flowSnapshot.explanation.decisions.length > 0}
                      <div class="detail-decisions">
                        {#each flowSnapshot.explanation.decisions as decision}
                          <div>
                            <Check class="decision-check" strokeWidth={2} /><span>{decision}</span>
                          </div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/if}
                {#if flowSnapshot.sourceRefs.length > 0}
                  <div class="detail-section">
                    <strong class="detail-label">Source references</strong>
                    {#each flowSnapshot.sourceRefs as sourceRef (sourceRef.id)}
                      <div class="source-ref-row">
                        <span>{sourceRef.label}</span>
                        <code>{sourceRef.path}:{sourceRef.startLine}-{sourceRef.endLine}</code>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            </ScrollArea>
          {:else if moduleSnapshot}
            <ScrollArea class="min-h-0 flex-1">
              <div class="detail-content">
                <header class="detail-heading">
                  <strong>{moduleSnapshot.name}</strong>
                  <span
                    >{moduleSnapshot.status} · r{store.selectedVersion?.revision} · {formatActivityTime(
                      store.selectedVersion?.createdAt ?? '',
                    )}</span
                  >
                </header>
                <dl class="module-fields">
                  <div><dt>Name</dt><dd>{moduleSnapshot.name}</dd></div>
                  <div><dt>Slug</dt><dd>{moduleSnapshot.slug}</dd></div>
                  <div><dt>Summary</dt><dd>{moduleSnapshot.summary || '—'}</dd></div>
                  <div><dt>Status</dt><dd>{moduleSnapshot.status}</dd></div>
                  <div><dt>Revision</dt><dd>{moduleSnapshot.revision ?? '—'}</dd></div>
                  <div><dt>Parent</dt><dd>{moduleSnapshot.parentId ?? '—'}</dd></div>
                  <div>
                    <dt>Updated</dt>
                    <dd>
                      {moduleSnapshot.updatedAt
                        ? formatActivityTime(moduleSnapshot.updatedAt)
                        : '—'}
                    </dd>
                  </div>
                </dl>
              </div>
            </ScrollArea>
          {:else}
            <div class="history-state app-state-fill">
              <Clock3 /><span>Select a version to inspect its snapshot.</span>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.history-dialog) {
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--dialog-shadow);
  }

  :global(.history-dialog .history-header) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .history-state {
    min-height: var(--state-min-height);
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
  }

  .history-state.error {
    color: var(--destructive);
  }

  .history-state :global(svg) {
    width: 15px;
    height: 15px;
  }

  .history-loading {
    display: grid;
    min-height: 260px;
    align-content: start;
  }

  .history-loading > div {
    display: grid;
    min-height: 54px;
    grid-template-columns: 56px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 18px;
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .history-body {
    display: grid;
    min-height: 0;
    max-height: min(560px, 72vh);
    grid-template-columns: 232px minmax(0, 1fr);
  }

  .history-list-pane {
    min-height: 0;
    border-right: 1px solid var(--border);
  }

  .history-list {
    display: grid;
  }

  .history-row {
    display: grid;
    min-height: 54px;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border: 0;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .history-row + .history-row {
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .history-row:hover {
    background: color-mix(in oklab, var(--foreground) 5%, transparent);
  }

  .history-row:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .history-row.selected {
    background: color-mix(in oklab, var(--foreground) 7%, transparent);
    box-shadow: inset 2px 0 0 var(--ring);
  }

  .history-row strong {
    font-family: var(--app-font-mono);
    font-size: 11px;
    font-weight: 600;
  }

  .history-row span {
    overflow: hidden;
    color: var(--muted-foreground);
    font-size: 11px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-row time {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .history-detail {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
  }

  .detail-loading {
    display: grid;
    gap: 12px;
    align-content: start;
    padding: 18px;
  }

  .detail-content {
    display: grid;
    gap: 14px;
    padding: 16px 18px 20px;
  }

  .detail-heading {
    display: grid;
    gap: 3px;
  }

  .detail-heading strong {
    font-size: 14px;
    font-weight: 620;
  }

  .detail-heading span {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .detail-summary {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 12.5px;
    line-height: 1.55;
  }

  .detail-diagram {
    min-height: 180px;
    overflow: hidden;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: var(--diagram-surface);
  }

  .detail-section {
    display: grid;
    gap: 8px;
  }

  .detail-section h2 {
    margin: 0;
    font-size: 13px;
    font-weight: 620;
  }

  .detail-section > p {
    margin: 0;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.6;
  }

  .detail-decisions {
    display: grid;
    gap: 8px;
  }

  .detail-decisions > div {
    display: grid;
    grid-template-columns: 14px 1fr;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.5;
  }

  :global(.decision-check) {
    width: 12px;
    margin-top: 1px;
    color: var(--status-success);
  }

  .detail-label {
    color: var(--muted-foreground);
    font-size: 11px;
    font-weight: 560;
  }

  .source-ref-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: baseline;
    gap: 10px;
  }

  .source-ref-row span {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .source-ref-row code {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .module-fields {
    display: grid;
    gap: 10px;
    margin: 0;
  }

  .module-fields > div {
    display: grid;
    grid-template-columns: 88px minmax(0, 1fr);
    gap: 12px;
  }

  .module-fields dt,
  .module-fields dd {
    margin: 0;
    font-size: 12px;
  }

  .module-fields dt {
    color: var(--muted-foreground);
  }

  .module-fields dd {
    overflow-wrap: anywhere;
  }

  @media (max-width: 720px) {
    .history-body {
      max-height: min(620px, 76vh);
      grid-template-columns: minmax(0, 1fr);
      grid-template-rows: minmax(0, 172px) minmax(0, 1fr);
    }

    .history-list-pane {
      border-right: 0;
      border-bottom: 1px solid var(--border);
    }
  }
</style>
