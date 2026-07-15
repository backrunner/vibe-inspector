<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import type { EntityStatus, ProjectStatus } from '$domain/entities';

  let { status } = $props<{ status: EntityStatus | ProjectStatus }>();

  const labels: Record<EntityStatus | ProjectStatus, string> = {
    draft: 'Draft',
    ready: 'Ready',
    stale: 'Stale',
    syncing: 'Syncing',
    'needs-review': 'Review',
    error: 'Error',
    archived: 'Archived',
  };

  const tone = $derived(
    status === 'ready'
      ? 'tone-success'
      : status === 'stale' || status === 'needs-review'
        ? 'tone-warning'
        : status === 'error'
          ? 'tone-error'
          : status === 'syncing'
            ? 'tone-info'
            : 'tone-neutral',
  );
</script>

<Badge
  variant="outline"
  class={`status-badge h-5 gap-1.5 rounded-md px-1.5 text-[11px] font-medium ${tone}`}
>
  <span class="size-1 rounded-full bg-current"></span>
  {labels[status as EntityStatus | ProjectStatus]}
</Badge>

<style>
  :global(.status-badge.tone-success) {
    border-color: color-mix(in oklab, var(--status-success) 28%, var(--border));
    color: var(--status-success);
  }

  :global(.status-badge.tone-warning) {
    border-color: color-mix(in oklab, var(--status-warning) 28%, var(--border));
    color: var(--status-warning);
  }

  :global(.status-badge.tone-error) {
    border-color: color-mix(in oklab, var(--status-error) 28%, var(--border));
    color: var(--status-error);
  }

  :global(.status-badge.tone-info) {
    border-color: color-mix(in oklab, var(--status-info) 28%, var(--border));
    color: var(--status-info);
  }

  :global(.status-badge.tone-neutral) {
    border-color: var(--border);
    color: var(--muted-foreground);
  }
</style>
