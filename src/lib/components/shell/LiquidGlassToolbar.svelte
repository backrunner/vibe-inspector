<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { generateLensMap } from '$lib/design/liquid-glass';

  let { children } = $props<{ children: Snippet }>();
  let toolbar: HTMLDivElement;
  let filterId = $state('');
  let mapDataUrl = $state('');
  let mapScale = $state(0);
  let mapWidth = $state(1);
  let mapHeight = $state(1);

  function trackHighlight(event: PointerEvent): void {
    const bounds = toolbar.getBoundingClientRect();
    toolbar.style.setProperty('--glass-pointer-x', `${event.clientX - bounds.left}px`);
    toolbar.style.setProperty('--glass-pointer-y', `${event.clientY - bounds.top}px`);
  }

  onMount(() => {
    let revision = 0;
    const updateMap = () => {
      const bounds = toolbar.getBoundingClientRect();
      const map = generateLensMap({
        width: bounds.width,
        height: bounds.height,
        radius: bounds.height / 2,
      });
      mapDataUrl = map.dataUrl;
      mapScale = map.scale;
      mapWidth = map.width;
      mapHeight = map.height;
      filterId = `vi-liquid-glass-${Date.now().toString(36)}-${revision++}`;
    };
    const observer = new ResizeObserver(updateMap);
    observer.observe(toolbar);
    updateMap();
    return () => observer.disconnect();
  });
</script>

<div
  bind:this={toolbar}
  class="liquid-glass-toolbar"
  role="group"
  aria-label="Global controls"
  onpointermove={trackHighlight}
  onpointerleave={() => {
    toolbar.style.removeProperty('--glass-pointer-x');
    toolbar.style.removeProperty('--glass-pointer-y');
  }}
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
  .liquid-glass-toolbar {
    --glass-pointer-x: 72%;
    --glass-pointer-y: 25%;

    position: relative;
    display: flex;
    isolation: isolate;
    align-items: center;
    padding: 2px;
    border-radius: calc(var(--radius-lg) + 2px);
    box-shadow: var(--glass-shadow);
    transition:
      transform var(--dur-micro) var(--ease-out),
      box-shadow var(--dur-micro) var(--ease-out);
  }

  :global(.liquid-glass-toolbar:has(button:active)) {
    transform: scale(0.985);
    box-shadow: var(--glass-shadow-active);
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

  .glass-highlight {
    background:
      radial-gradient(
        circle 28px at var(--glass-pointer-x) var(--glass-pointer-y),
        var(--glass-interaction-light),
        transparent 72%
      ),
      linear-gradient(145deg, var(--glass-leading-light), transparent 42%),
      linear-gradient(325deg, var(--glass-trailing-light), transparent 38%);
    box-shadow: inset 0 0 0 1px var(--glass-rim);
    transition: background-position var(--dur-micro) var(--ease-out);
  }

  .glass-content {
    position: relative;
    display: flex;
    z-index: 1;
    align-items: center;
    gap: 1px;
  }

  :global(.liquid-glass-toolbar button) {
    color: color-mix(in oklab, var(--foreground) 84%, transparent);
  }

  :global(.liquid-glass-toolbar button:hover) {
    background: color-mix(in oklab, var(--foreground) 9%, transparent);
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

  @media (prefers-reduced-motion: reduce) {
    .liquid-glass-toolbar,
    .glass-highlight {
      transition: none;
    }

    :global(.liquid-glass-toolbar:has(button:active)) {
      transform: none;
    }
  }
</style>
