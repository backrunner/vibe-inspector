<script lang="ts">
  import { onMount } from 'svelte';
  import { Skeleton } from '$lib/components/ui/skeleton';

  const maxCachedDiagrams = 12;
  const diagramCache = new Map<string, string>();

  let {
    definition,
    theme,
    label = 'Business flow diagram',
  } = $props<{
    definition: string;
    theme: 'dark' | 'light';
    label?: string;
  }>();

  let mermaid = $state<typeof import('mermaid').default>();
  let renderPrefix = $state('');
  let svg = $state('');
  let renderError = $state('');
  let renderSequence = 0;
  let initializedTheme = '';

  function token(name: string): string {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    if (!value) throw new Error(`Missing diagram theme token: ${name}`);
    return value;
  }

  async function renderDiagram(currentDefinition: string, currentTheme: 'dark' | 'light') {
    if (!mermaid || !renderPrefix) return;
    const sequence = ++renderSequence;
    const cacheKey = `${currentTheme}\n${currentDefinition}`;
    renderError = '';
    const cachedSvg = diagramCache.get(cacheKey);
    if (cachedSvg) {
      diagramCache.delete(cacheKey);
      diagramCache.set(cacheKey, cachedSvg);
      svg = cachedSvg;
      return;
    }

    if (initializedTheme !== currentTheme) {
      mermaid.initialize({
        startOnLoad: false,
        securityLevel: 'strict',
        suppressErrorRendering: true,
        theme: 'base',
        themeVariables: {
          darkMode: currentTheme === 'dark',
          background: token('--diagram-surface'),
          primaryColor: token('--diagram-node'),
          primaryTextColor: token('--diagram-text'),
          primaryBorderColor: token('--diagram-border'),
          secondaryColor: token('--diagram-node'),
          tertiaryColor: token('--diagram-node'),
          lineColor: token('--diagram-line'),
          edgeLabelBackground: token('--diagram-surface'),
          fontFamily: token('--app-font-sans'),
          fontSize: token('--diagram-font-size'),
        },
        themeCSS: `.nodeLabel { font-weight: 520; } .edgeLabel { font-size: ${token('--diagram-edge-font-size')}; }`,
        flowchart: {
          curve: 'basis',
          htmlLabels: false,
          nodeSpacing: 18,
          rankSpacing: 28,
          padding: 6,
          useMaxWidth: true,
        },
      });
      initializedTheme = currentTheme;
    }

    try {
      const result = await mermaid.render(`${renderPrefix}-${sequence}`, currentDefinition);
      if (sequence !== renderSequence) return;
      svg = result.svg;
      diagramCache.set(cacheKey, result.svg);
      if (diagramCache.size > maxCachedDiagrams) {
        diagramCache.delete(diagramCache.keys().next().value ?? '');
      }
    } catch (error) {
      if (sequence !== renderSequence) return;
      svg = '';
      renderError = error instanceof Error ? error.message : 'The diagram could not be rendered.';
    }
  }

  onMount(() => {
    let disposed = false;
    renderPrefix = `vi-mermaid-${crypto.randomUUID()}`;

    void import('mermaid').then((module) => {
      if (disposed) return;
      mermaid = module.default;
    });

    return () => {
      disposed = true;
      renderSequence += 1;
    };
  });

  $effect(() => {
    const currentDefinition = definition;
    const currentTheme = theme;
    if (mermaid && renderPrefix) void renderDiagram(currentDefinition, currentTheme);
  });
</script>

<div class="mermaid-diagram" role="img" aria-label={label}>
  {#if renderError}
    <div class="diagram-state error" role="alert">{renderError}</div>
  {:else if svg}
    <div class="diagram-svg">{@html svg}</div>
  {:else}
    <div class="diagram-skeleton" aria-label="Rendering flow diagram" aria-busy="true">
      <Skeleton class="h-full w-full rounded-lg" />
      <div class="diagram-skeleton-nodes" aria-hidden="true">
        <Skeleton class="h-12 w-24 rounded-lg bg-background" />
        <Skeleton class="h-12 w-28 rounded-lg bg-background" />
        <Skeleton class="h-12 w-24 rounded-lg bg-background" />
      </div>
    </div>
  {/if}
</div>

<style>
  .mermaid-diagram,
  .diagram-svg {
    display: grid;
    width: 100%;
    height: 100%;
    min-height: 0;
    place-items: center;
  }

  .diagram-svg {
    padding: 12px 16px;
  }

  .diagram-svg :global(svg) {
    display: block;
    width: 100%;
    max-width: none !important;
    height: auto;
    max-height: 100%;
  }

  .diagram-svg :global(.node rect),
  .diagram-svg :global(.node .label-container) {
    rx: var(--radius-compact);
    ry: var(--radius-compact);
  }

  .diagram-svg :global(.flowchart-link) {
    stroke-width: 1.25px;
  }

  .diagram-state {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .diagram-state.error {
    max-width: 46ch;
    color: var(--destructive);
    text-align: center;
  }

  .diagram-skeleton {
    display: grid;
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-height: 0;
    padding: 12px 16px;
  }

  .diagram-skeleton > :global(*) {
    grid-area: 1 / 1;
  }

  .diagram-skeleton-nodes {
    display: flex;
    align-items: center;
    justify-content: space-evenly;
    gap: 18px;
    padding: 24px;
  }
</style>
