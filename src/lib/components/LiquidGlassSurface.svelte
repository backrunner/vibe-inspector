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

  // Neutral margin around the lens so rim refraction samples painted pixels, not void.
  const refractionMargin = 10;
  // Chromatic fringe: red/blue channels refract slightly wider/narrower than green.
  const chromaStrength = $derived(variant === 'field' ? 0.06 : 0.1);
  const scaleRed = $derived(mapScale * (1 + chromaStrength));
  const scaleBlue = $derived(mapScale * (1 - chromaStrength));

  onMount(() => {
    const updateMap = () => {
      const bounds = surface.getBoundingClientRect();
      const map = generateLensMap({
        width: bounds.width,
        height: bounds.height,
        radius: bounds.height / 2,
        margin: refractionMargin,
        depth: variant === 'field' ? 5.5 : 9,
        opacity: variant === 'field' ? 0.72 : 0.88,
      });
      mapDataUrl = map.dataUrl;
      mapScale = map.scale;
      mapWidth = map.width;
      mapHeight = map.height;
      // Safari caches filter output by filter id; a fresh id forces it to read the new map.
      filterId = `vi-liquid-glass-${crypto.randomUUID()}`;
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
          x="-4"
          y="-4"
          width={mapWidth + 8}
          height={mapHeight + 8}
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
            scale={scaleRed}
            xChannelSelector="R"
            yChannelSelector="G"
            result="refract-red"
          />
          <feDisplacementMap
            in="SourceGraphic"
            in2="lens-map"
            scale={mapScale}
            xChannelSelector="R"
            yChannelSelector="G"
            result="refract-green"
          />
          <feDisplacementMap
            in="SourceGraphic"
            in2="lens-map"
            scale={scaleBlue}
            xChannelSelector="R"
            yChannelSelector="G"
            result="refract-blue"
          />
          <feColorMatrix
            in="refract-red"
            type="matrix"
            values="1 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0"
            result="channel-red"
          />
          <feColorMatrix
            in="refract-green"
            type="matrix"
            values="0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 1 0"
            result="channel-green"
          />
          <feColorMatrix
            in="refract-blue"
            type="matrix"
            values="0 0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 0 1 0"
            result="channel-blue"
          />
          <feBlend in="channel-red" in2="channel-green" mode="screen" result="channels-rg" />
          <feBlend in="channels-rg" in2="channel-blue" mode="screen" result="refracted" />
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
    <span class="glass-specular"></span>
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
    transition: box-shadow var(--dur-short) var(--ease-out);
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
  .glass-tint,
  .glass-highlight,
  .glass-specular {
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
    position: absolute;
    inset: -10px;
    border-radius: inherit;
    background: var(--app-backdrop);
    background-attachment: fixed;
    background-size: 100vw 100vh;
    transform: translateZ(0);
    pointer-events: none;
  }

  .glass-tint {
    background: var(--glass-tint);
  }

  [data-variant='field'] .glass-tint {
    background: color-mix(in oklab, var(--glass-tint) 72%, var(--input) 28%);
  }

  .glass-highlight {
    background:
      linear-gradient(145deg, var(--glass-leading-light), transparent 46%),
      linear-gradient(325deg, var(--glass-trailing-light), transparent 42%);
    box-shadow: inset 0 0 0 1px var(--glass-rim);
  }

  [data-variant='field'] .glass-highlight {
    background:
      linear-gradient(
        155deg,
        color-mix(in oklab, var(--glass-leading-light) 72%, transparent),
        transparent 48%
      ),
      linear-gradient(
        335deg,
        color-mix(in oklab, var(--glass-trailing-light) 68%, transparent),
        transparent 44%
      );
  }

  /* Directional rim light, confined to a 1px edge band so the glass reads as
     a thin lens edge instead of a thick bezel. */
  .glass-specular {
    padding: 1px;
    background: conic-gradient(
      from 258deg at 50% 50%,
      transparent 0deg,
      var(--glass-specular) 58deg,
      color-mix(in oklab, var(--glass-specular) 30%, transparent) 112deg,
      transparent 168deg,
      transparent 200deg,
      var(--glass-specular-shade) 237deg,
      transparent 292deg
    );
    -webkit-mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    -webkit-mask-composite: xor;
    mask:
      linear-gradient(#000 0 0) content-box,
      linear-gradient(#000 0 0);
    mask-composite: exclude;
  }

  [data-variant='field'] .glass-specular {
    opacity: 0.6;
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
