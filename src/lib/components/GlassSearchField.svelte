<script lang="ts">
  import { Search } from '@lucide/svelte';
  import { Input } from '$lib/components/ui/input';
  import { cn } from '$lib/utils.js';
  import LiquidGlassSurface from '$components/LiquidGlassSurface.svelte';

  let {
    value = $bindable(''),
    placeholder,
    ariaLabel,
    class: className = '',
  } = $props<{
    value?: string;
    placeholder: string;
    ariaLabel?: string;
    class?: string;
  }>();
</script>

<LiquidGlassSurface variant="field" class={cn('glass-search-field', className)}>
  <Search class="search-icon" strokeWidth={1.8} aria-hidden="true" />
  <Input
    bind:value
    {placeholder}
    aria-label={ariaLabel ?? placeholder}
    class="h-8 border-0 bg-transparent pr-3 pl-8 text-body [box-shadow:none] focus-visible:border-transparent focus-visible:ring-0 dark:bg-transparent"
  />
</LiquidGlassSurface>

<style>
  :global(.glass-search-field) {
    position: relative;
  }

  :global(.glass-search-field .search-icon) {
    position: absolute;
    top: 50%;
    left: 10px;
    z-index: 2;
    width: 14px;
    height: 14px;
    color: var(--muted-foreground);
    pointer-events: none;
    transform: translateY(-50%);
  }

  :global(.glass-search-field:has(input:focus-visible) .search-icon) {
    color: var(--foreground);
  }
</style>
