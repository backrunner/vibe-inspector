<script lang="ts">
  import { Clock3, Database, TerminalSquare, TriangleAlert } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { formatActivityTime } from '$lib/adapters/activity';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
</script>

<Dialog.Root bind:open={store.activityOpen}>
  <Dialog.Content class="activity-dialog gap-0 overflow-hidden p-0 sm:max-w-[680px]">
    <Dialog.Header class="border-b px-5 py-4 text-left">
      <Dialog.Title class="text-heading">Activity</Dialog.Title>
      <Dialog.Description class="text-rail">
        {store.selectedProject?.name ?? 'No project selected'}
      </Dialog.Description>
    </Dialog.Header>

    {#if store.activityStatus === 'loading'}
      <div class="activity-loading" aria-label="Loading activity" aria-busy="true">
        <Skeleton class="h-10 w-full rounded-none" />
        {#each [1, 2, 3, 4] as row}
          <div>
            <Skeleton class="size-2 rounded-full" /><span
              ><Skeleton class="h-3 w-40" /><Skeleton class="h-2.5 w-24" /></span
            ><Skeleton class="h-3 w-20" />
          </div>
        {/each}
      </div>
    {:else if store.activityStatus === 'error'}
      <div class="activity-state error app-state-fill">
        <TriangleAlert /><span>{store.activityError}</span>
      </div>
    {:else}
      <ScrollArea class="max-h-[min(620px,72vh)]">
        <section class="activity-section">
          <header>
            <TerminalSquare /><strong>Agent commands</strong><span>{store.commandRuns.length}</span>
          </header>
          {#if store.commandRuns.length === 0}
            <p class="empty-row app-state-fill">No command activity for this project.</p>
          {:else}
            <div class="activity-rows">
              {#each store.commandRuns as run (run.requestId)}
                <div class="activity-row">
                  <span class:review={run.status === 'needs-review'} class="status-mark"></span>
                  <div>
                    <strong>{run.command}</strong>
                    <span>{run.actor}</span>
                  </div>
                  <div class="row-meta">
                    <em>{run.status}</em>
                    <time><Clock3 />{formatActivityTime(run.updatedAt)}</time>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>

        <section class="activity-section audit-section">
          <header>
            <Database /><strong>MCP audit</strong><span>{store.auditEvents.length}</span>
          </header>
          {#if store.auditEvents.length === 0}
            <p class="empty-row app-state-fill">No MCP calls recorded for this project.</p>
          {:else}
            <div class="activity-rows">
              {#each store.auditEvents as event (event.id)}
                <div class="activity-row">
                  <span class:error={event.result !== 'ok'} class="status-mark"></span>
                  <div>
                    <strong>{event.tool}</strong>
                    <span
                      >{event.entityIds.length > 0
                        ? event.entityIds.join(', ')
                        : event.sessionId}</span
                    >
                  </div>
                  <div class="row-meta">
                    <em>{event.durationMs} ms</em>
                    <time><Clock3 />{formatActivityTime(event.createdAt)}</time>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </section>
      </ScrollArea>
    {/if}
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.activity-dialog) {
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--dialog-shadow);
  }

  .activity-state {
    min-height: var(--state-min-height);
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
  }

  .activity-loading {
    display: grid;
    min-height: 260px;
    align-content: start;
  }

  .activity-loading > div {
    display: grid;
    min-height: 54px;
    grid-template-columns: 8px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 18px;
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .activity-loading > div > span {
    display: grid;
    gap: 5px;
  }

  .activity-state.error {
    color: var(--destructive);
  }

  .activity-state :global(svg) {
    width: 15px;
    height: 15px;
  }

  .activity-section + .activity-section {
    border-top: 1px solid var(--border);
  }

  .activity-section > header {
    display: grid;
    min-height: 42px;
    grid-template-columns: 16px 1fr auto;
    align-items: center;
    gap: 8px;
    padding: 0 18px;
    color: var(--muted-foreground);
    background: color-mix(in oklab, var(--muted) 45%, transparent);
    box-shadow: var(--inset-shadow);
  }

  .activity-section > header :global(svg) {
    width: 14px;
    height: 14px;
  }

  .activity-section > header strong {
    color: var(--foreground);
    font-size: 12px;
    font-weight: 600;
  }

  .activity-section > header span {
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .activity-rows {
    display: grid;
  }

  .activity-row {
    display: grid;
    min-height: 54px;
    grid-template-columns: 7px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 18px;
  }

  .activity-row + .activity-row {
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .activity-row > div:not(.row-meta) {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .activity-row strong {
    overflow: hidden;
    font-size: 12px;
    font-weight: 570;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .activity-row span,
  .activity-row time,
  .activity-row em {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
    font-style: normal;
  }

  .activity-row > div > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-mark {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--status-success);
  }

  .status-mark.review {
    background: var(--status-warning);
  }

  .status-mark.error {
    background: var(--status-error);
  }

  .row-meta {
    display: grid;
    justify-items: end;
    gap: 3px;
  }

  .row-meta time {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .row-meta time :global(svg) {
    width: 10px;
    height: 10px;
  }

  .empty-row {
    margin: 0;
    min-height: 132px;
    color: var(--muted-foreground);
    font-size: 12px;
  }

  .activity-section:has(> .empty-row) {
    min-height: 174px;
  }
</style>
