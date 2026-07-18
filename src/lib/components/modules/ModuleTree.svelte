<script lang="ts">
  import { tick } from 'svelte';
  import { ChevronDown, ChevronRight, Package, SearchX, Workflow } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { inspectorStore } from '$stores/inspector.svelte';
  import { buildModuleTree, moduleAncestorIds, type ModuleTreeRow } from '$domain/module-tree';

  const store = inspectorStore;
  let treeRoot = $state<HTMLElement>();
  let expanded = $state(
    new Set(['module-devex', 'module-inspector-ui', 'module-checkout', 'module-orders']),
  );
  let expandedSelectionId = '';

  const model = $derived(
    buildModuleTree(store.selectedProject?.modules ?? [], expanded, store.searchQuery),
  );
  const rows = $derived(model.rows);
  const selectedRowId = $derived(
    store.selectedFlowId ? `flow:${store.selectedFlowId}` : `module:${store.selectedModuleId}`,
  );

  $effect(() => {
    const selectedModuleId = store.selectedModuleId;
    const modules = store.selectedProject?.modules ?? [];
    if (!selectedModuleId || selectedModuleId === expandedSelectionId) return;
    expandedSelectionId = selectedModuleId;
    const next = new Set(expanded);
    for (const id of moduleAncestorIds(modules, selectedModuleId)) next.add(id);
    expanded = next;
  });

  function rowId(row: ModuleTreeRow): string {
    return row.kind === 'module' ? `module:${row.module.id}` : `flow:${row.flow.id}`;
  }

  function parentRowId(row: ModuleTreeRow): string | undefined {
    return row.parentId ? `module:${row.parentId}` : undefined;
  }

  function isOpen(moduleId: string): boolean {
    return Boolean(store.searchQuery.trim()) || expanded.has(moduleId);
  }

  function hasChildren(row: Extract<ModuleTreeRow, { kind: 'module' }>): boolean {
    return Boolean(row.module.children?.length || row.module.flows.length);
  }

  function toggle(moduleId: string) {
    expanded = new Set(expanded);
    if (expanded.has(moduleId)) expanded.delete(moduleId);
    else expanded.add(moduleId);
  }

  async function focusRow(id: string) {
    await tick();
    treeRoot?.querySelector<HTMLElement>(`[data-tree-id="${CSS.escape(id)}"]`)?.focus();
  }

  function handleKeydown(event: KeyboardEvent, row: ModuleTreeRow) {
    const index = rows.findIndex((entry) => rowId(entry) === rowId(row));
    if (index < 0) return;

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const offset = event.key === 'ArrowDown' ? 1 : -1;
      const target = rows[Math.min(rows.length - 1, Math.max(0, index + offset))];
      if (target) void focusRow(rowId(target));
      return;
    }

    if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      const target = event.key === 'Home' ? rows[0] : rows.at(-1);
      if (target) void focusRow(rowId(target));
      return;
    }

    if (event.key === 'ArrowRight' && row.kind === 'module' && hasChildren(row)) {
      event.preventDefault();
      if (!isOpen(row.module.id)) toggle(row.module.id);
      else if (rows[index + 1]) void focusRow(rowId(rows[index + 1]));
      return;
    }

    if (event.key === 'ArrowLeft') {
      if (row.kind === 'module' && isOpen(row.module.id) && hasChildren(row)) {
        event.preventDefault();
        toggle(row.module.id);
      } else {
        const parent = parentRowId(row);
        if (parent) {
          event.preventDefault();
          void focusRow(parent);
        }
      }
    }
  }
</script>

<div
  class:empty={rows.length === 0}
  class="tree"
  role="tree"
  aria-label="Repository modules"
  bind:this={treeRoot}
>
  {#if rows.length === 0}
    <div class="module-empty app-state-fill">
      <SearchX class="size-4" strokeWidth={1.7} aria-hidden="true" />
      <strong>{store.searchQuery.trim() ? 'No matching modules' : 'No modules available'}</strong>
      <span
        >{store.searchQuery.trim()
          ? 'Try a broader filter.'
          : 'Architecture appears after a project is registered.'}</span
      >
      {#if store.searchQuery.trim()}
        <Button variant="ghost" size="xs" onclick={() => (store.searchQuery = '')}
          >Clear filter</Button
        >
      {/if}
    </div>
  {:else}
    {#each rows as row (rowId(row))}
      {#if row.kind === 'module'}
        <div
          class:active={row.module.id === store.selectedModuleId}
          class="module-row tree-row"
          style:--depth={row.depth}
        >
          {#if row.depth > 0}<span class="module-guide" aria-hidden="true"></span>{/if}
          {#if hasChildren(row)}
            <button
              type="button"
              class="disclosure"
              tabindex="-1"
              aria-label={`${isOpen(row.module.id) ? 'Collapse' : 'Expand'} ${row.module.name}`}
              onclick={() => toggle(row.module.id)}
            >
              {#if isOpen(row.module.id)}
                <ChevronDown class="size-3" strokeWidth={1.8} />
              {:else}
                <ChevronRight class="size-3" strokeWidth={1.8} />
              {/if}
            </button>
          {:else}
            <span class="disclosure" aria-hidden="true"></span>
          {/if}
          <button
            type="button"
            class="module-select"
            role="treeitem"
            aria-level={row.depth + 1}
            aria-expanded={hasChildren(row) ? isOpen(row.module.id) : undefined}
            aria-selected={row.module.id === store.selectedModuleId && !store.selectedFlowId}
            tabindex={rowId(row) === selectedRowId ? 0 : -1}
            data-tree-id={rowId(row)}
            onclick={() => store.selectModule(row.module.id)}
            onkeydown={(event) => handleKeydown(event, row)}
          >
            <Package class="size-3.5 shrink-0" strokeWidth={1.7} />
            <span class="min-w-0 flex-1 truncate">{row.module.name}</span>
            {#if (model.flowCounts.get(row.module.id) ?? 0) > 0}
              <small title={`${model.flowCounts.get(row.module.id)} business flows`}>
                {model.flowCounts.get(row.module.id)}
              </small>
            {/if}
          </button>
        </div>
      {:else}
        <button
          type="button"
          class:selected={row.flow.id === store.selectedFlowId}
          class="flow-row tree-row"
          style:--depth={row.depth}
          role="treeitem"
          aria-level={row.depth + 1}
          aria-selected={row.flow.id === store.selectedFlowId}
          aria-current={row.flow.id === store.selectedFlowId ? 'page' : undefined}
          tabindex={rowId(row) === selectedRowId ? 0 : -1}
          data-tree-id={rowId(row)}
          onclick={() => store.selectFlow(row.module.id, row.flow.id)}
          onkeydown={(event) => handleKeydown(event, row)}
        >
          <span class="flow-branch" aria-hidden="true"></span>
          <Workflow class="size-3.5 shrink-0" strokeWidth={1.7} />
          <span class="min-w-0 flex-1 truncate">{row.flow.name}</span>
        </button>
      {/if}
    {/each}
  {/if}
</div>

<style>
  .tree {
    display: grid;
    align-content: start;
    gap: 2px;
    height: 100%;
    min-height: 0;
    padding: 0 8px 16px;
  }

  .tree.empty {
    align-content: stretch;
    grid-template-rows: minmax(0, 1fr);
    padding: 0;
  }

  .module-empty {
    min-height: 0;
    gap: 8px;
    padding-bottom: calc(var(--state-padding) + var(--module-empty-chrome-offset, 0px));
    color: var(--muted-foreground);
    font-size: 11.5px;
  }

  .module-empty > strong {
    color: var(--foreground);
    font-weight: 600;
  }

  .module-empty > span {
    max-width: 22ch;
    line-height: 1.45;
  }

  .tree-row {
    position: relative;
    min-width: 0;
    color: var(--muted-foreground);
  }

  .module-row {
    display: flex;
    min-height: 32px;
    align-items: center;
    padding-left: calc(4px + var(--depth) * 18px);
    border-radius: var(--radius-md);
  }

  .module-row:hover,
  .module-row:focus-within {
    color: var(--foreground);
    background: color-mix(in oklab, var(--muted) 62%, transparent);
  }

  .module-row.active {
    color: var(--foreground);
  }

  .module-guide {
    position: absolute;
    top: -3px;
    bottom: -3px;
    left: calc(13px + (var(--depth) - 1) * 18px);
    width: 1px;
    background: var(--border);
  }

  .disclosure {
    display: grid;
    width: 20px;
    height: 26px;
    flex: 0 0 20px;
    place-items: center;
    border: 0;
    color: var(--muted-foreground);
    background: transparent;
  }

  button.disclosure:hover {
    color: var(--foreground);
  }

  .module-select {
    display: flex;
    min-width: 0;
    height: 30px;
    flex: 1;
    align-items: center;
    gap: 7px;
    padding: 0 7px 0 2px;
    border: 0;
    color: inherit;
    background: transparent;
    text-align: left;
    font-size: 12.5px;
    font-weight: 530;
  }

  .module-select small {
    min-width: 14px;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    text-align: right;
  }

  .flow-row {
    display: flex;
    width: 100%;
    min-height: 30px;
    align-items: center;
    gap: 7px;
    padding: 0 8px 0 calc(16px + var(--depth) * 18px);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    text-align: left;
    font-size: 12px;
    transition:
      color var(--dur-micro) var(--ease-out),
      background-color var(--dur-micro) var(--ease-out),
      border-color var(--dur-micro) var(--ease-out);
  }

  .flow-row:hover {
    color: var(--foreground);
    background: color-mix(in oklab, var(--muted) 62%, transparent);
  }

  .flow-row.selected {
    border-color: color-mix(in oklab, var(--signal) 30%, var(--border));
    color: var(--foreground);
    background: color-mix(in oklab, var(--signal) 8%, transparent);
    box-shadow: var(--selected-shadow);
  }

  .flow-row:focus-visible,
  .module-select:focus-visible,
  .disclosure:focus-visible {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .flow-branch {
    position: absolute;
    top: -3px;
    bottom: 50%;
    left: calc(-5px + var(--depth) * 18px);
    width: 10px;
    border-bottom: 1px solid var(--border);
    border-left: 1px solid var(--border);
    border-bottom-left-radius: 5px;
  }
</style>
