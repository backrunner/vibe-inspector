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
  variant={selected ? 'secondary' : 'ghost'}
  {size}
  class={cn('segmented-control-item', className)}
  aria-pressed={selected}
  data-segmented-control-item
  data-selected={selected ? '' : undefined}
  {...restProps}
>
  {@render children?.()}
</Button>
