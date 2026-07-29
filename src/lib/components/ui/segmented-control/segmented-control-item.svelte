<script lang="ts">
  import { Button, type ButtonProps, type ButtonSize } from '$lib/components/ui/button';
  import { cn } from '$lib/utils.js';

  type SegmentedControlItemProps = Omit<ButtonProps, 'variant'> & {
    selected?: boolean;
    size?: ButtonSize;
  };

  let {
    ref = $bindable(null),
    class: className,
    selected = false,
    size = 'xs',
    children,
    ...restProps
  }: SegmentedControlItemProps = $props();
</script>

<Button
  bind:ref
  variant="ghost"
  {size}
  class={cn('segmented-control-item', className)}
  aria-pressed={selected}
  data-segmented-control-item
  data-selected={selected ? '' : undefined}
  {...restProps}
>
  {@render children?.()}
</Button>

<style>
  /* macOS-style segmented behavior: the selected segment is a raised chip
     that keeps its elevation on hover; unselected segments get a soft tint
     instead of the harsh ghost flip. */
  :global(.segmented-control-item) {
    color: var(--muted-foreground);
    transition:
      color var(--dur-micro) var(--ease-out),
      background-color var(--dur-micro) var(--ease-out),
      box-shadow var(--dur-micro) var(--ease-out);
  }

  :global(.segmented-control-item:hover) {
    color: var(--foreground);
    background: color-mix(in oklab, var(--foreground) 6%, transparent);
  }

  :global(.segmented-control-item[data-selected]) {
    color: var(--foreground);
    background: var(--card);
    box-shadow: var(--control-shadow);
  }

  :global(.segmented-control-item[data-selected]:hover) {
    background: var(--card);
    box-shadow: var(--item-shadow);
  }
</style>
