<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton';
</script>

<div class="viewer-skeleton" aria-label="Loading source range" aria-busy="true">
  <header>
    <div class="file-heading"><Skeleton class="size-6 rounded-md" /><Skeleton class="h-3 w-48" /></div>
    <Skeleton class="h-3 w-24" />
  </header>
  <div class="code-lines">
    {#each [1, 2, 3, 4, 5, 6, 7] as line}
      <Skeleton
        class={`h-3 rounded-sm ${line % 3 === 0 ? 'w-[68%]' : line % 3 === 1 ? 'w-[88%]' : 'w-[54%]'}`}
      />
    {/each}
  </div>
  <footer><Skeleton class="h-2.5 w-36" /><Skeleton class="h-2.5 w-24" /></footer>
</div>

<style>
  .viewer-skeleton {
    display: grid;
    height: 100%;
    min-height: 220px;
    grid-template-rows: 44px minmax(0, 1fr) 28px;
    background: var(--card);
  }

  .viewer-skeleton > header,
  .viewer-skeleton > footer,
  .file-heading {
    display: flex;
    align-items: center;
  }

  .viewer-skeleton > header,
  .viewer-skeleton > footer {
    justify-content: space-between;
    gap: 12px;
    padding: 0 13px;
    border-bottom: 1px solid var(--border);
  }

  .viewer-skeleton > footer {
    border-top: 1px solid var(--border);
    border-bottom: 0;
  }

  .file-heading {
    gap: 9px;
  }

  .code-lines {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 18px 20px;
  }
</style>
