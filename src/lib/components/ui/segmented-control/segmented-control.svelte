<script lang="ts">
  import { cn, type WithElementRef } from '$lib/utils.js';
  import type { HTMLAttributes } from 'svelte/elements';

  let {
    ref = $bindable(null),
    class: className,
    role = 'group',
    children,
    ...restProps
  }: WithElementRef<HTMLAttributes<HTMLDivElement>, HTMLDivElement> = $props();
</script>

<div
  bind:this={ref}
  data-slot="segmented-control"
  {role}
  class={cn('segmented-control', className)}
  {...restProps}
>
  {@render children?.()}
</div>

<style>
  .segmented-control {
    display: inline-flex;
    width: max-content;
    flex: 0 0 auto;
    align-items: center;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: color-mix(in oklab, var(--muted) 58%, transparent);
    box-shadow: var(--inset-shadow);
  }
</style>
