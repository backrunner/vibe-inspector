<script lang="ts">
  import { Skeleton } from '$lib/components/ui/skeleton';
</script>

<div class="workspace-skeleton" aria-label="Loading inspector data" aria-busy="true">
  <header>
    <div>
      <Skeleton class="h-3 w-40" />
      <Skeleton class="h-6 w-72" />
      <Skeleton class="h-3.5 w-[32rem] max-w-full" />
    </div>
    <Skeleton class="h-8 w-24 rounded-md" />
  </header>

  <section class="overview-placeholder">
    <div class="diagram-placeholder">
      <div class="diagram-toolbar">
        <Skeleton class="h-3.5 w-28" />
        <Skeleton class="h-7 w-20 rounded-md" />
      </div>
      <div class="diagram-nodes">
        {#each [1, 2, 3, 4] as node}
          <Skeleton class="h-12 w-28 rounded-lg" />
        {/each}
      </div>
    </div>
    <div class="rationale-placeholder">
      <Skeleton class="h-11 w-full rounded-none" />
      <div>
        <Skeleton class="h-5 w-44" />
        <Skeleton class="h-3.5 w-full" />
        <Skeleton class="h-3.5 w-11/12" />
        <Skeleton class="h-3.5 w-4/5" />
      </div>
    </div>
  </section>

  <section class="source-placeholder">
    <div class="source-labels">
      <Skeleton class="h-3.5 w-28" />
      <Skeleton class="h-3 w-52" />
    </div>
    <div class="source-surface-placeholder">
      <div class="tree-placeholder">
        <Skeleton class="h-8 w-full rounded-none" />
        {#each [1, 2, 3, 4] as row}
          <Skeleton class={`h-7 rounded-md ${row % 2 === 0 ? 'w-[82%]' : 'w-[92%]'}`} />
        {/each}
      </div>
      <div class="editor-placeholder">
        <Skeleton class="h-11 w-full rounded-none" />
        <div>
          {#each [1, 2, 3, 4, 5, 6] as line}
            <Skeleton
              class={`h-3 rounded-sm ${line % 3 === 0 ? 'w-[72%]' : line % 3 === 1 ? 'w-[88%]' : 'w-[58%]'}`}
            />
          {/each}
        </div>
      </div>
    </div>
  </section>
</div>

<style>
  .workspace-skeleton {
    display: grid;
    box-sizing: border-box;
    min-height: 0;
    flex: 1;
    grid-template-rows: auto minmax(190px, 0.9fr) minmax(262px, 1.15fr);
    gap: 12px;
    overflow: hidden;
    padding: 16px 20px;
    border: 1px solid var(--panel-edge);
    border-radius: var(--radius-xl);
    background: var(--workspace-panel-surface);
    box-shadow: var(--panel-shadow);
  }

  .workspace-skeleton > header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
  }

  .workspace-skeleton > header > div {
    display: grid;
    gap: 9px;
  }

  .overview-placeholder {
    display: grid;
    min-height: 0;
    grid-template-columns: minmax(300px, 1fr) 280px;
    gap: 12px;
  }

  .diagram-placeholder,
  .rationale-placeholder,
  .source-surface-placeholder {
    overflow: hidden;
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: var(--panel-shadow);
  }

  .diagram-placeholder {
    display: grid;
    min-height: 0;
    grid-template-rows: 44px 1fr;
  }

  .diagram-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 12px;
    background: var(--panel-header-surface);
    box-shadow: 0 12px 22px -24px color-mix(in oklab, var(--foreground) 22%, transparent);
  }

  .diagram-nodes {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-evenly;
    gap: 18px;
    padding: 24px;
  }

  .rationale-placeholder {
    display: grid;
    grid-template-rows: 46px 1fr;
  }

  .rationale-placeholder > div {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 16px;
  }

  .source-placeholder {
    display: grid;
    min-height: 0;
    grid-template-rows: 34px minmax(0, 1fr);
    gap: 8px;
  }

  .source-labels {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .source-surface-placeholder {
    display: grid;
    min-height: 0;
    grid-template-columns: 220px minmax(0, 1fr);
  }

  .tree-placeholder {
    display: grid;
    align-content: start;
    gap: 8px;
    padding: 0 8px 8px;
    box-shadow: var(--pane-separation-shadow);
    z-index: 1;
  }

  .tree-placeholder > :global(:first-child) {
    margin: 0 -8px 2px;
  }

  .editor-placeholder {
    display: grid;
    min-height: 0;
    grid-template-rows: 44px 1fr;
  }

  .editor-placeholder > div {
    display: grid;
    align-content: start;
    gap: 10px;
    padding: 18px 20px;
  }

  @media (max-width: 1200px) {
    .workspace-skeleton {
      padding: 14px;
    }

    .overview-placeholder {
      grid-template-columns: minmax(0, 1fr) 270px;
    }

    .source-surface-placeholder {
      grid-template-columns: 220px minmax(0, 1fr);
    }
  }
</style>
