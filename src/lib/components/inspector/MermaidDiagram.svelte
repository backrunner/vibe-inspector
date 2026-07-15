<script lang="ts">
  import { onMount } from 'svelte';

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

  function token(name: string, fallback: string): string {
    if (typeof document === 'undefined') return fallback;
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || fallback;
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
          background: token('--diagram-surface', currentTheme === 'dark' ? '#18181b' : '#ffffff'),
          primaryColor: token('--diagram-node', currentTheme === 'dark' ? '#27272a' : '#fafafa'),
          primaryTextColor: token(
            '--diagram-text',
            currentTheme === 'dark' ? '#fafafa' : '#18181b',
          ),
          primaryBorderColor: token(
            '--diagram-border',
            currentTheme === 'dark' ? '#52525b' : '#d4d4d8',
          ),
          secondaryColor: token('--diagram-node', currentTheme === 'dark' ? '#27272a' : '#fafafa'),
          tertiaryColor: token('--diagram-node', currentTheme === 'dark' ? '#27272a' : '#fafafa'),
          lineColor: token('--diagram-line', currentTheme === 'dark' ? '#a1a1aa' : '#71717a'),
          edgeLabelBackground: token(
            '--diagram-surface',
            currentTheme === 'dark' ? '#18181b' : '#ffffff',
          ),
          fontFamily: '-apple-system, BlinkMacSystemFont, SF Pro Text, Helvetica Neue, sans-serif',
          fontSize: '11px',
        },
        themeCSS: '.nodeLabel { font-weight: 520; } .edgeLabel { font-size: 10px; }',
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
    <div class="diagram-state">Rendering flow diagram</div>
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
    rx: 6px;
    ry: 6px;
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
</style>
