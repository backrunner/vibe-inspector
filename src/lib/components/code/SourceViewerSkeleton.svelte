<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton';
</script>

<div class="viewer-skeleton" aria-label="Loading source file" aria-busy="true">
  <header>
    <div class="file-heading">
      <Skeleton class="size-6 rounded-md" /><Skeleton class="h-3 w-48" />
    </div>
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
    position: relative;
    justify-content: space-between;
    gap: 12px;
    padding: 0 13px;
    background: var(--panel-header-surface);
    box-shadow: 0 10px 20px -23px color-mix(in oklab, var(--foreground) 24%, transparent);
  }

  .viewer-skeleton > header::after,
  .viewer-skeleton > footer::after {
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 12px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .viewer-skeleton > footer {
    background: var(--panel-footer-surface);
    box-shadow: 0 -10px 20px -23px color-mix(in oklab, var(--foreground) 24%, transparent);
  }

  .viewer-skeleton > footer::after {
    top: 0;
    bottom: auto;
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
