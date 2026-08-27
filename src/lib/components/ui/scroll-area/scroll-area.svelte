<script lang="ts">
  import { ScrollArea as ScrollAreaPrimitive } from 'bits-ui';
  import { onMount } from 'svelte';
  import { Scrollbar } from './index.js';
  import { scrollShadowState, type ScrollShadowState } from '$lib/domain/scroll-shadows.js';
  import { cn, type WithoutChild } from '$lib/utils.js';

  let {
    ref = $bindable(null),
    viewportRef = $bindable(null),
    class: className,
    orientation = 'vertical',
    scrollbarXClasses = '',
    scrollbarYClasses = '',
    viewportProps,
    children,
    ...restProps
  }: WithoutChild<ScrollAreaPrimitive.RootProps> & {
    orientation?: 'vertical' | 'horizontal' | 'both' | undefined;
    scrollbarXClasses?: string | undefined;
    scrollbarYClasses?: string | undefined;
    viewportRef?: HTMLElement | null;
    viewportProps?: ScrollAreaPrimitive.ViewportProps | undefined;
  } = $props();

  let shadows = $state<ScrollShadowState>({
    top: false,
    right: false,
    bottom: false,
    left: false,
  });

  onMount(() => {
    const viewport = viewportRef;
    if (!viewport) return;

    let frame = 0;
    const update = () => {
      shadows = scrollShadowState(viewport);
    };
    const scheduleUpdate = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(update);
    };
    const resizeObserver = new ResizeObserver(scheduleUpdate);
    const observeContent = () => {
      resizeObserver.disconnect();
      resizeObserver.observe(viewport);
      for (const child of viewport.children) resizeObserver.observe(child);
      scheduleUpdate();
    };
    const mutationObserver = new MutationObserver(observeContent);

    viewport.addEventListener('scroll', scheduleUpdate, { passive: true });
    mutationObserver.observe(viewport, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    observeContent();

    return () => {
      cancelAnimationFrame(frame);
      viewport.removeEventListener('scroll', scheduleUpdate);
      resizeObserver.disconnect();
      mutationObserver.disconnect();
    };
  });
</script>

<ScrollAreaPrimitive.Root
  bind:ref
  data-slot="scroll-area"
  class={cn('relative', className)}
  {...restProps}
>
  <ScrollAreaPrimitive.Viewport
    {...viewportProps}
    bind:ref={viewportRef}
    data-slot="scroll-area-viewport"
    class={cn(
      'cn-scroll-area-viewport focus-visible:ring-ring size-full rounded-[inherit] outline-none focus-visible:ring-[3px] focus-visible:outline-1',
      viewportProps?.class,
    )}
  >
    {@render children?.()}
  </ScrollAreaPrimitive.Viewport>
  {#if orientation === 'vertical' || orientation === 'both'}
    <Scrollbar orientation="vertical" class={scrollbarYClasses} />
  {/if}
  {#if orientation === 'horizontal' || orientation === 'both'}
    <Scrollbar orientation="horizontal" class={scrollbarXClasses} />
  {/if}
  <ScrollAreaPrimitive.Corner />
  <span class:visible={shadows.top} class="scroll-shadow scroll-shadow-top" aria-hidden="true"
  ></span>
  <span class:visible={shadows.right} class="scroll-shadow scroll-shadow-right" aria-hidden="true"
  ></span>
  <span class:visible={shadows.bottom} class="scroll-shadow scroll-shadow-bottom" aria-hidden="true"
  ></span>
  <span class:visible={shadows.left} class="scroll-shadow scroll-shadow-left" aria-hidden="true"
  ></span>
</ScrollAreaPrimitive.Root>

<style>
  .scroll-shadow {
    position: absolute;
    z-index: 4;
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur-micro) var(--ease-out);
  }

  .scroll-shadow.visible {
    opacity: 1;
  }

  .scroll-shadow-top,
  .scroll-shadow-bottom {
    right: 0;
    left: 0;
    height: 16px;
  }

  .scroll-shadow-left,
  .scroll-shadow-right {
    top: 0;
    bottom: 0;
    width: 16px;
  }

  .scroll-shadow-top {
    top: 0;
    background: linear-gradient(to bottom, var(--scroll-shadow), transparent);
  }

  .scroll-shadow-right {
    right: 0;
    background: linear-gradient(to left, var(--scroll-shadow), transparent);
  }

  .scroll-shadow-bottom {
    bottom: 0;
    background: linear-gradient(to top, var(--scroll-shadow), transparent);
  }

  .scroll-shadow-left {
    left: 0;
    background: linear-gradient(to right, var(--scroll-shadow), transparent);
  }

  @media (prefers-reduced-motion: reduce) {
    .scroll-shadow {
      transition: none;
    }
  }
</style>
