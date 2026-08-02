<script lang="ts">
  import { ChevronDown, CircleAlert, TriangleAlert } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import * as SegmentedControl from '$lib/components/ui/segmented-control';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { formatActivityTime, formatRelativeActivity } from '$lib/adapters/activity';
  import { formatIssueLocation, isIssueClosed } from '$lib/adapters/issues';
  import type { IssueStatus } from '$domain/entities';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  let statusFilter = $state<'all' | 'open' | 'closed'>('all');
  const visibleIssues = $derived(
    store.issues.filter((issue) =>
      statusFilter === 'all' ? true : statusFilter === 'open' ? !isIssueClosed(issue) : isIssueClosed(issue),
    ),
  );
  const statusLabels: Record<IssueStatus, string> = {
    open: 'Open',
    resolved: 'Resolved',
    wontfix: "Won't fix",
  };
  const statusChoices: IssueStatus[] = ['open', 'resolved', 'wontfix'];
</script>

<Dialog.Root bind:open={store.issuesOpen}>
  <Dialog.Content class="issues-dialog gap-0 overflow-hidden p-0 sm:max-w-[680px]">
    <Dialog.Header class="issues-header border-b px-5 py-4 text-left">
      <div class="min-w-0">
        <Dialog.Title class="text-heading">Issues</Dialog.Title>
        <Dialog.Description class="text-rail">
          {store.selectedModule?.name ?? 'No module selected'}
        </Dialog.Description>
      </div>
      <SegmentedControl.Root aria-label="Filter issues by status">
        <SegmentedControl.Item
          class="min-w-[52px]"
          selected={statusFilter === 'all'}
          onclick={() => (statusFilter = 'all')}>All</SegmentedControl.Item
        >
        <SegmentedControl.Item
          class="min-w-[52px]"
          selected={statusFilter === 'open'}
          onclick={() => (statusFilter = 'open')}>Open</SegmentedControl.Item
        >
        <SegmentedControl.Item
          class="min-w-[60px]"
          selected={statusFilter === 'closed'}
          onclick={() => (statusFilter = 'closed')}>Closed</SegmentedControl.Item
        >
      </SegmentedControl.Root>
    </Dialog.Header>

    {#if store.issuesStatus === 'loading'}
      <div class="issues-loading" aria-label="Loading issues" aria-busy="true">
        {#each [1, 2, 3, 4] as row}
          <div>
            <Skeleton class="h-4 w-9" /><span><Skeleton class="h-3 w-48" /><Skeleton
                class="h-2.5 w-32"
              /></span><Skeleton class="h-5 w-20" />
          </div>
        {/each}
      </div>
    {:else if store.issuesStatus === 'error'}
      <div class="issues-state error app-state-fill">
        <TriangleAlert /><span>{store.issuesError}</span>
      </div>
    {:else}
      {#if store.issuesError}
        <div class="issues-banner" role="alert">
          <TriangleAlert /><span>{store.issuesError}</span>
        </div>
      {/if}
      {#if visibleIssues.length === 0}
        <div class="issues-state app-state-fill">
          <CircleAlert /><span
            >{store.issues.length === 0
              ? 'No issues recorded for this module.'
              : 'No issues match this filter.'}</span
          >
        </div>
      {:else}
        <ScrollArea class="max-h-[min(560px,72vh)]">
          <div class="issues-list">
            {#each visibleIssues as issue (issue.id)}
              <div class="issue-row">
                <Badge
                  variant={issue.severity === 'p0' ? 'destructive' : 'secondary'}
                  class={issue.severity === 'p1' ? 'severity-p1' : ''}
                >
                  {issue.severity.toUpperCase()}
                </Badge>
                <div class="issue-main">
                  <strong>{issue.title}</strong>
                  {#if issue.description}<span class="issue-description">{issue.description}</span
                    >{/if}
                  <span class="issue-meta">
                    {#if formatIssueLocation(issue)}<code>{formatIssueLocation(issue)}</code
                      > · {/if}{issue.actor} ·
                    <time title={formatActivityTime(issue.updatedAt)}
                      >{formatRelativeActivity(issue.updatedAt)}</time
                    >
                  </span>
                </div>
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger>
                    {#snippet child({ props })}
                      <Button
                        {...props}
                        variant="ghost"
                        size="xs"
                        class="status-trigger"
                        data-status={issue.status}
                        aria-label={`Change status of ${issue.title}`}
                      >
                        {statusLabels[issue.status]}<ChevronDown class="size-3" strokeWidth={1.8} />
                      </Button>
                    {/snippet}
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end">
                    {#each statusChoices as status}
                      <DropdownMenu.Item
                        disabled={status === issue.status}
                        onclick={() => void store.setIssueStatus(issue, status)}
                      >
                        {statusLabels[status]}
                      </DropdownMenu.Item>
                    {/each}
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </div>
            {/each}
          </div>
        </ScrollArea>
      {/if}
    {/if}
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.issues-dialog) {
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--dialog-shadow);
  }

  :global(.issues-dialog .issues-header) {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  :global(.issues-dialog .severity-p1) {
    border-color: transparent;
    color: var(--status-warning);
    background: color-mix(in oklab, var(--status-warning) 14%, transparent);
  }

  .issues-state {
    min-height: var(--state-min-height);
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 12px;
  }

  .issues-state.error {
    color: var(--destructive);
  }

  .issues-state :global(svg) {
    width: 15px;
    height: 15px;
  }

  .issues-loading {
    display: grid;
    min-height: 260px;
    align-content: start;
  }

  .issues-loading > div {
    display: grid;
    min-height: 54px;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 18px;
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .issues-loading > div > span {
    display: grid;
    gap: 5px;
  }

  .issues-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 18px;
    color: var(--destructive);
    background: color-mix(in oklab, var(--destructive) 8%, transparent);
    font-size: 12px;
  }

  .issues-banner :global(svg) {
    width: 13px;
    height: 13px;
    flex: 0 0 auto;
  }

  .issues-list {
    display: grid;
  }

  .issue-row {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) auto;
    align-items: start;
    gap: 10px;
    padding: 10px 14px 10px 18px;
  }

  .issue-row + .issue-row {
    border-top: 1px solid color-mix(in oklab, var(--border) 70%, transparent);
  }

  .issue-main {
    display: grid;
    min-width: 0;
    gap: 3px;
  }

  .issue-main strong {
    overflow: hidden;
    font-size: 12.5px;
    font-weight: 570;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issue-description {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    color: var(--muted-foreground);
    font-size: 11.5px;
    line-height: 1.5;
  }

  .issue-meta {
    overflow: hidden;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issue-meta code {
    font-family: inherit;
  }

  :global(.issues-dialog .status-trigger) {
    gap: 4px;
    color: var(--muted-foreground);
    font-size: 11px;
  }

  :global(.issues-dialog .status-trigger[data-status='open']) {
    color: var(--status-warning);
  }

  :global(.issues-dialog .status-trigger[data-status='resolved']) {
    color: var(--status-success);
  }
</style>
