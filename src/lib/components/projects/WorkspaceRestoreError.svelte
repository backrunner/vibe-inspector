<script lang="ts">
  import { RefreshCw, TriangleAlert } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
</script>

<main class="restore-error" aria-labelledby="restore-error-title">
  <section role="alert">
    <span class="state-icon"><TriangleAlert strokeWidth={1.7} /></span>
    <div>
      <h1 id="restore-error-title">Workspace unavailable</h1>
      <p>VibeInspector could not load the local project registry. No projects were changed.</p>
    </div>
    <pre>{store.workspaceError}</pre>
    <Button size="sm" onclick={() => void store.restoreRegisteredProjects()}>
      <RefreshCw data-icon="inline-start" />Retry
    </Button>
  </section>
</main>

<style>
  .restore-error {
    display: grid;
    width: 100%;
    height: 100%;
    min-height: 0;
    place-items: center;
    background: var(--workspace-surface);
  }

  section {
    display: grid;
    width: min(520px, calc(100% - 48px));
    justify-items: start;
    gap: 14px;
    padding-bottom: 4vh;
  }

  section > div {
    display: grid;
    gap: 6px;
  }

  .state-icon {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 1px solid color-mix(in oklab, var(--status-error) 28%, var(--border));
    border-radius: var(--radius-md);
    color: var(--status-error);
    background: color-mix(in oklab, var(--status-error) 7%, var(--muted));
  }

  .state-icon :global(svg) {
    width: 16px;
  }

  h1 {
    margin: 0;
    font-size: 22px;
    line-height: 1.2;
    font-weight: 650;
    letter-spacing: 0;
  }

  p {
    max-width: 52ch;
    margin: 0;
    color: var(--muted-foreground);
    font-size: 13px;
    line-height: 1.55;
  }

  pre {
    width: 100%;
    margin: 0;
    padding: 10px 12px;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--status-error);
    background: color-mix(in oklab, var(--muted) 62%, transparent);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }
</style>
