<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton';
  import InspectorSkeleton from '$components/inspector/InspectorSkeleton.svelte';
</script>

<div class="shell-skeleton" aria-label="Loading workspace" aria-busy="true">
  <aside class="project-placeholder">
    <header><Skeleton class="h-3.5 w-16" /><Skeleton class="size-7 rounded-md" /></header>
    <div>
      {#each [1, 2, 3] as row}
        <div class="project-row-placeholder">
          <Skeleton class="size-7 rounded-md" />
          <span><Skeleton class="h-3 w-28" /><Skeleton class="h-2.5 w-24" /></span>
        </div>
      {/each}
    </div>
  </aside>
  <aside class="module-placeholder">
    <header>
      <span><Skeleton class="h-3.5 w-24" /><Skeleton class="h-2.5 w-20" /></span><Skeleton
        class="size-7 rounded-md"
      />
    </header>
    <Skeleton class="mx-3 mt-3 h-8 w-auto rounded-lg" />
    <div class="module-rows">
      {#each [1, 2, 3, 4, 5] as row}
        <Skeleton class={`h-7 rounded-md ${row % 2 === 0 ? 'ml-5 w-[74%]' : 'w-[86%]'}`} />
      {/each}
    </div>
  </aside>
  <InspectorSkeleton />
</div>

<style>
  .shell-skeleton {
    display: grid;
    min-width: 0;
    min-height: 0;
    flex: 1;
    grid-template-columns: 216px 280px minmax(0, 1fr);
    background: transparent;
  }

  .project-placeholder,
  .module-placeholder {
    min-width: 0;
    border-right: 1px solid var(--border);
  }

  .project-placeholder {
    background: transparent;
  }

  .module-placeholder {
    background: transparent;
  }

  .project-placeholder > header,
  .module-placeholder > header {
    display: flex;
    height: 44px;
    align-items: center;
    justify-content: space-between;
    padding: 0 10px 0 16px;
    border-bottom: 1px solid var(--border);
  }

  .module-placeholder > header > span {
    display: grid;
    gap: 4px;
  }

  .project-placeholder > div,
  .module-rows {
    display: grid;
    gap: 7px;
    padding: 10px 8px;
  }

  .project-row-placeholder {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 9px;
    padding: 3px 2px;
  }

  .project-row-placeholder > span {
    display: grid;
    gap: 5px;
  }

  .module-rows {
    padding: 16px 12px;
  }

  @media (max-width: 1200px) {
    .shell-skeleton {
      grid-template-columns: 196px 252px minmax(0, 1fr);
    }
  }
</style>
