<script lang="ts">
  import type { Component } from 'svelte';
  import { Button } from '$lib/components/ui/button';

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

<Button variant="ghost" size="icon-sm" aria-label={label} title={label} {onclick}>
  <Icon class="size-3.5" strokeWidth={1.8} />
</Button>
