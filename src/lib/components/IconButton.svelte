<script lang="ts">
  import type { Component } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Tooltip from '$lib/components/ui/tooltip';

  let {
    label,
    icon: Icon,
    onclick,
  } = $props<{
    label: string;
    icon: Component;
    onclick?: (event: MouseEvent) => void;
  }>();
</script>

<Tooltip.Root>
  <Tooltip.Trigger>
    {#snippet child({ props })}
      <Button {...props} variant="ghost" size="icon-sm" aria-label={label} {onclick}>
        <Icon class="size-3.5" strokeWidth={1.8} />
      </Button>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Content side="bottom" sideOffset={4}>
    <span class="text-caption">{label}</span>
  </Tooltip.Content>
</Tooltip.Root>
