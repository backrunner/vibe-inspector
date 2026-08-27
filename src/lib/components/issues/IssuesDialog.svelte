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
      statusFilter === 'all'
        ? true
        : statusFilter === 'open'
          ? !isIssueClosed(issue)
          : isIssueClosed(issue),
    ),
  );
  const openIssueCount = $derived(store.issues.filter((issue) => !isIssueClosed(issue)).length);
  const closedIssueCount = $derived(store.issues.length - openIssueCount);
  const statusLabels: Record<IssueStatus, string> = {
    open: 'Open',
    resolved: 'Resolved',
    wontfix: "Won't fix",
  };
  const statusChoices: IssueStatus[] = ['open', 'resolved', 'wontfix'];
</script>

<Dialog.Root bind:open={store.issuesOpen}>
  <Dialog.Content class="issues-dialog gap-0 overflow-hidden p-0 sm:max-w-[720px]">
    <Dialog.Header class="issues-header text-left">
      <div class="issues-heading-row">
        <div class="issues-heading">
          <span class="issues-heading-icon"
            ><CircleAlert strokeWidth={1.8} aria-hidden="true" /></span
          >
          <div class="min-w-0">
            <Dialog.Title class="issues-title">Issues</Dialog.Title>
            <Dialog.Description class="issues-module">
              {store.selectedModule?.name ?? 'No module selected'}
            </Dialog.Description>
          </div>
        </div>
        <div class="issues-overview" aria-label={`${openIssueCount} open issues`}>
          <strong>{openIssueCount}</strong><span>open</span>
        </div>
      </div>
      <div class="issues-filter-row">
        <span>Status</span>
        <SegmentedControl.Root class="issues-filter" aria-label="Filter issues by status">
          <SegmentedControl.Item
            class="issue-filter"
            selected={statusFilter === 'all'}
            onclick={() => (statusFilter = 'all')}
            ><span>All</span><small>{store.issues.length}</small></SegmentedControl.Item
          >
          <SegmentedControl.Item
            class="issue-filter"
            selected={statusFilter === 'open'}
            onclick={() => (statusFilter = 'open')}
            ><span>Open</span><small>{openIssueCount}</small></SegmentedControl.Item
          >
          <SegmentedControl.Item
            class="issue-filter"
            selected={statusFilter === 'closed'}
            onclick={() => (statusFilter = 'closed')}
            ><span>Closed</span><small>{closedIssueCount}</small></SegmentedControl.Item
          >
        </SegmentedControl.Root>
      </div>
    </Dialog.Header>

    {#if store.issuesStatus === 'loading'}
      <div class="issues-loading" aria-label="Loading issues" aria-busy="true">
        {#each [1, 2, 3, 4] as row}
          <div>
            <Skeleton class="h-4 w-9" /><span
              ><Skeleton class="h-3 w-48" /><Skeleton class="h-2.5 w-32" /></span
            ><Skeleton class="h-5 w-20" />
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
          <CircleAlert aria-hidden="true" />
          <strong
            >{store.issues.length === 0
              ? 'No issues recorded'
              : `No ${statusFilter} issues`}</strong
          >
          <span
            >{store.issues.length === 0
              ? 'Agent review findings for this module will appear here.'
              : 'Choose another status to inspect the remaining issues.'}</span
          >
          {#if store.issues.length > 0 && statusFilter !== 'all'}
            <Button variant="ghost" size="xs" onclick={() => (statusFilter = 'all')}
              >Show all issues</Button
            >
          {/if}
        </div>
      {:else}
        <ScrollArea
          class="issues-scroll"
          viewportProps={{ tabindex: 0, role: 'region', 'aria-label': 'Issue list' }}
        >
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
                    {#if formatIssueLocation(issue)}<code>{formatIssueLocation(issue)}</code> ·
                    {/if}{issue.actor} ·
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
                        <span class="status-dot" aria-hidden="true"></span>
                        <span>{statusLabels[issue.status]}</span>
                        <ChevronDown class="size-3" strokeWidth={1.8} aria-hidden="true" />
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
    position: relative;
    display: grid;
    gap: 14px;
    padding: 18px 20px 14px;
    background: var(--panel-header-surface);
  }

  :global(.issues-dialog .issues-header)::after {
    position: absolute;
    right: 18px;
    bottom: 0;
    left: 18px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .issues-heading-row,
  .issues-heading,
  .issues-filter-row {
    display: flex;
    align-items: center;
  }

  .issues-heading-row {
    justify-content: space-between;
    gap: 20px;
  }

  .issues-heading {
    min-width: 0;
    gap: 10px;
  }

  .issues-heading-icon {
    display: grid;
    width: 30px;
    height: 30px;
    flex: 0 0 30px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    background: var(--muted);
    box-shadow: var(--control-shadow);
  }

  .issues-heading-icon :global(svg) {
    width: 15px;
    height: 15px;
  }

  :global(.issues-dialog .issues-title) {
    font-size: 16px;
    line-height: 1.2;
    font-weight: 640;
    letter-spacing: 0;
  }

  :global(.issues-dialog .issues-module) {
    overflow: hidden;
    margin-top: 3px;
    color: var(--muted-foreground);
    font-size: 11.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issues-overview {
    display: flex;
    flex: 0 0 auto;
    align-items: baseline;
    gap: 5px;
    color: var(--muted-foreground);
  }

  .issues-overview strong {
    color: var(--foreground);
    font-family: var(--app-font-mono);
    font-size: 13px;
    font-weight: 600;
  }

  .issues-overview span,
  .issues-filter-row > span {
    font-size: 10.5px;
  }

  .issues-filter-row {
    justify-content: space-between;
    gap: 12px;
  }

  .issues-filter-row > span {
    color: var(--muted-foreground);
    font-weight: 560;
  }

  :global(.issues-dialog .issue-filter) {
    min-width: 72px;
    gap: 6px;
  }

  :global(.issues-dialog .issue-filter small) {
    min-width: 14px;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 9.5px;
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

  .issues-state > strong {
    color: var(--foreground);
    font-size: 12px;
    font-weight: 600;
  }

  .issues-state > span {
    max-width: 36ch;
    line-height: 1.5;
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

  :global(.issues-scroll) {
    min-height: 220px;
    max-height: min(560px, 72vh);
  }

  .issue-row {
    display: grid;
    grid-template-columns: 42px minmax(0, 1fr) 112px;
    align-items: start;
    gap: 10px;
    padding: 12px 16px 12px 18px;
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
    font-size: 12.5px;
    font-weight: 570;
    line-height: 1.4;
    overflow-wrap: anywhere;
  }

  .issue-description {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
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
    width: 112px;
    justify-content: flex-start;
    gap: 6px;
    color: var(--muted-foreground);
    font-size: 11px;
  }

  :global(.issues-dialog .status-trigger > span:nth-child(2)) {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  :global(.issues-dialog .status-dot) {
    width: 6px;
    height: 6px;
    flex: 0 0 6px;
    border-radius: 50%;
    background: currentColor;
  }

  :global(.issues-dialog .status-trigger[data-status='open']) {
    color: var(--status-warning);
  }

  :global(.issues-dialog .status-trigger[data-status='resolved']) {
    color: var(--status-success);
  }
</style>
