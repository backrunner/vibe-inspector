<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { generateLensMap } from '$lib/design/liquid-glass';

  type LiquidGlassVariant = 'toolbar' | 'field';

  let {
    children,
    variant = 'toolbar',
    role,
    ariaLabel,
    class: className = '',
  } = $props<{
    children: Snippet;
    variant?: LiquidGlassVariant;
    role?: string;
    ariaLabel?: string;
    class?: string;
  }>();
  let surface: HTMLDivElement;
  let filterId = $state('');
  let mapDataUrl = $state('');
  let mapScale = $state(0);
  let mapWidth = $state(1);
  let mapHeight = $state(1);

  onMount(() => {
    filterId = `vi-liquid-glass-${crypto.randomUUID()}`;
    const updateMap = () => {
      const bounds = surface.getBoundingClientRect();
      const map = generateLensMap({
        width: bounds.width,
        height: bounds.height,
        radius: bounds.height / 2,
        depth: variant === 'field' ? 5.5 : 8,
        opacity: variant === 'field' ? 0.72 : 0.88,
      });
      mapDataUrl = map.dataUrl;
      mapScale = map.scale;
      mapWidth = map.width;
      mapHeight = map.height;
    };
    const observer = new ResizeObserver(updateMap);
    observer.observe(surface);
    updateMap();
    return () => observer.disconnect();
  });
</script>

<div
  bind:this={surface}
  class={`liquid-glass-surface ${className}`}
  data-variant={variant}
  {role}
  aria-label={ariaLabel}
>
  <svg class="glass-definitions" aria-hidden="true">
    <defs>
      {#if mapDataUrl && filterId}
        <filter
          id={filterId}
          x="-8"
          y="-8"
          width={mapWidth + 16}
          height={mapHeight + 16}
          filterUnits="userSpaceOnUse"
          color-interpolation-filters="sRGB"
        >
          <feImage
            href={mapDataUrl}
            x="0"
            y="0"
            width={mapWidth}
            height={mapHeight}
            preserveAspectRatio="none"
            result="lens-map"
          />
          <feDisplacementMap
            in="SourceGraphic"
            in2="lens-map"
            scale={mapScale}
            xChannelSelector="R"
            yChannelSelector="G"
            result="refracted"
          />
          <feComposite in="refracted" in2="lens-map" operator="in" result="masked-refraction" />
          <feMerge>
            <feMergeNode in="SourceGraphic" />
            <feMergeNode in="masked-refraction" />
          </feMerge>
        </filter>
      {/if}
    </defs>
  </svg>
  <span class="glass-surface" aria-hidden="true">
    <span class="glass-refraction" style:filter={filterId ? `url(#${filterId})` : undefined}></span>
    <span class="glass-tint"></span>
    <span class="glass-highlight"></span>
  </span>
  <span class="glass-content">{@render children()}</span>
</div>

<style>
  .liquid-glass-surface {
    position: relative;
    display: flex;
    isolation: isolate;
    align-items: center;
  }

  .liquid-glass-surface[data-variant='toolbar'] {
    padding: 2px;
    border-radius: calc(var(--radius-lg) + 2px);
    box-shadow: var(--glass-shadow);
  }

  .liquid-glass-surface[data-variant='field'] {
    width: 100%;
    height: 34px;
    border-radius: var(--radius-lg);
    box-shadow: var(--control-shadow);
  }

  .liquid-glass-surface[data-variant='field']:hover {
    box-shadow: var(--item-shadow);
  }

  .liquid-glass-surface[data-variant='field']:focus-within {
    outline: 2px solid var(--ring);
    outline-offset: 1px;
    box-shadow: var(--item-shadow);
  }

  .glass-definitions {
    position: absolute;
    width: 0;
    height: 0;
    overflow: hidden;
  }

  .glass-surface,
  .glass-refraction,
  .glass-tint,
  .glass-highlight {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
  }

  .glass-surface {
    z-index: -1;
    overflow: hidden;
  }

  .glass-refraction {
    background: var(--app-backdrop);
    background-attachment: fixed;
    background-size: 100vw 100vh;
    transform: translateZ(0);
  }

  .glass-tint {
    background: var(--glass-tint);
  }

  [data-variant='field'] .glass-tint {
    background: color-mix(in oklab, var(--glass-tint) 72%, var(--input) 28%);
  }

  .glass-highlight {
    background:
      linear-gradient(145deg, var(--glass-leading-light), transparent 42%),
      linear-gradient(325deg, var(--glass-trailing-light), transparent 38%);
    box-shadow: inset 0 0 0 1px var(--glass-rim);
  }

  [data-variant='field'] .glass-highlight {
    background:
      linear-gradient(
        155deg,
        color-mix(in oklab, var(--glass-leading-light) 72%, transparent),
        transparent 46%
      ),
      linear-gradient(
        335deg,
        color-mix(in oklab, var(--glass-trailing-light) 68%, transparent),
        transparent 42%
      );
  }

  .glass-content {
    position: relative;
    display: flex;
    z-index: 1;
    align-items: center;
    gap: 1px;
  }

  [data-variant='field'] .glass-content {
    width: 100%;
  }

  @media (prefers-reduced-transparency: reduce) {
    .glass-refraction {
      display: none;
    }

    .glass-tint {
      background: var(--popover);
    }
  }

  @media (prefers-contrast: more) {
    .glass-highlight {
      box-shadow: inset 0 0 0 1px var(--foreground);
    }
  }
</style>
