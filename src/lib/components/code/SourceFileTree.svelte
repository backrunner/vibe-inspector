<script lang="ts" module>
  function flattenVisible(
    nodes: SourceTreeNode[],
    expanded: Record<string, boolean>,
    depth = 0,
    parentId?: string,
  ): VisibleNode[] {
    const rows: VisibleNode[] = [];
    for (const node of nodes) {
      rows.push({ kind: 'node', node, depth, parentId });
      if (node.kind === 'folder' && (expanded[node.id] ?? true)) {
        rows.push(...flattenVisible(node.children, expanded, depth + 1, node.id));
      }
      if (node.kind === 'file' && node.refs.length > 1 && (expanded[node.id] ?? true)) {
        rows.push(
          ...node.refs.map((ref) => ({
            kind: 'range' as const,
            id: `range:${ref.id}`,
            file: node,
            ref,
            depth: depth + 1,
            parentId: node.id,
          })),
        );
      }
    }
    return rows;
  }

  function findSelectedFileId(nodes: SourceTreeNode[], selectedRefId: string): string {
    for (const node of nodes) {
      if (node.kind === 'file' && node.refs.some((ref) => ref.id === selectedRefId)) return node.id;
      if (node.kind === 'folder') {
        const child = findSelectedFileId(node.children, selectedRefId);
        if (child) return child;
      }
    }
    return '';
  }
</script>

<script lang="ts">
  import { tick } from 'svelte';
  import { ChevronRight, FileCode2, Folder, FolderOpen } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { cn } from '$lib/utils.js';
  import type { SourceRef } from '$domain/entities';
  import {
    buildSourceTree,
    sourceTreeFileCount,
    type SourceTreeFile,
    type SourceTreeNode,
  } from '$domain/source-tree';

  type VisibleNode =
    | {
        kind: 'node';
        node: SourceTreeNode;
        depth: number;
        parentId?: string;
      }
    | {
        kind: 'range';
        id: string;
        file: SourceTreeFile;
        ref: SourceRef;
        depth: number;
        parentId: string;
      };

  let {
    sourceRefs,
    selectedRefId,
    onselect,
  }: {
    sourceRefs: SourceRef[];
    selectedRefId: string;
    onselect: (refId: string) => void;
  } = $props();

  let expanded = $state<Record<string, boolean>>({});
  let treeRoot = $state<HTMLElement>();
  const tree = $derived(buildSourceTree(sourceRefs));
  const fileCount = $derived(sourceTreeFileCount(tree));
  const selectedFileId = $derived(findSelectedFileId(tree, selectedRefId));
  const visibleNodes = $derived(flattenVisible(tree, expanded));
  const activeTreeItemId = $derived.by(() => {
    const selectedRange = visibleNodes.find(
      (item) => item.kind === 'range' && item.ref.id === selectedRefId,
    );
    if (selectedRange) return itemId(selectedRange);

    const selectedFile = visibleNodes.find(
      (item) =>
        item.kind === 'node' &&
        item.node.kind === 'file' &&
        item.node.refs.some((ref) => ref.id === selectedRefId),
    );
    return selectedFile ? itemId(selectedFile) : visibleNodes[0] ? itemId(visibleNodes[0]) : '';
  });

  function isExpanded(id: string): boolean {
    return expanded[id] ?? true;
  }

  function toggleNode(id: string) {
    expanded = { ...expanded, [id]: !isExpanded(id) };
  }

  function selectFile(node: SourceTreeNode) {
    if (node.kind !== 'file') return;
    if (node.refs.length > 1) {
      toggleNode(node.id);
      return;
    }
    if (node.refs[0]) onselect(node.refs[0].id);
  }

  function itemId(item: VisibleNode): string {
    return item.kind === 'range' ? item.id : item.node.id;
  }

  function isExpandable(item: VisibleNode): boolean {
    return (
      item.kind === 'node' &&
      ((item.node.kind === 'folder' && item.node.children.length > 0) ||
        (item.node.kind === 'file' && item.node.refs.length > 1))
    );
  }

  async function focusNode(id: string) {
    await tick();
    treeRoot?.querySelector<HTMLElement>(`[data-tree-id="${CSS.escape(id)}"]`)?.focus();
  }

  function handleKeydown(event: KeyboardEvent, item: VisibleNode) {
    const index = visibleNodes.findIndex((entry) => itemId(entry) === itemId(item));
    if (index < 0) return;

    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const offset = event.key === 'ArrowDown' ? 1 : -1;
      const target = visibleNodes[Math.min(visibleNodes.length - 1, Math.max(0, index + offset))];
      if (target) void focusNode(itemId(target));
      return;
    }

    if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      const target = event.key === 'Home' ? visibleNodes[0] : visibleNodes.at(-1);
      if (target) void focusNode(itemId(target));
      return;
    }

    if (event.key === 'ArrowRight' && isExpandable(item) && item.kind === 'node') {
      event.preventDefault();
      if (!isExpanded(item.node.id)) toggleNode(item.node.id);
      else if (visibleNodes[index + 1]) void focusNode(itemId(visibleNodes[index + 1]));
      return;
    }

    if (event.key === 'ArrowLeft') {
      if (isExpandable(item) && item.kind === 'node' && isExpanded(item.node.id)) {
        event.preventDefault();
        toggleNode(item.node.id);
      } else if (item.parentId) {
        event.preventDefault();
        void focusNode(item.parentId);
      }
    }
  }

  function rangeLabel(refs: SourceRef[]): string {
    if (refs.length > 1) return `${refs.length} ranges`;
    const ref = refs[0];
    if (ref?.origin === 'codegraph') return codegraphRelationshipLabel(ref);
    return ref ? `L${ref.startLine}-${ref.endLine}` : '';
  }

  function codegraphRelationshipLabel(ref: SourceRef): string {
    if (ref.codegraphRelationship === 'dependency') return 'dependency';
    if (ref.codegraphRelationship === 'dependent') return 'dependent';
    if (ref.codegraphRelationship === 'both') return 'both';
    return 'related';
  }

  function fileTitle(file: SourceTreeFile): string {
    const relation = file.refs.find((ref) => ref.origin === 'codegraph');
    if (!relation) return file.path;
    const direction =
      relation.codegraphRelationship === 'dependency'
        ? 'Referenced source depends on this file'
        : relation.codegraphRelationship === 'dependent'
          ? 'This file depends on referenced source'
          : relation.codegraphRelationship === 'both'
            ? 'Dependencies exist in both directions'
            : 'Related by CodeGraph';
    const kinds = relation.codegraphKinds?.length
      ? ` via ${relation.codegraphKinds.join(', ')}`
      : '';
    return `${file.path}\n${direction}${kinds}`;
  }

  function rangeDetail(ref: SourceRef): string {
    if (ref.origin === 'codegraph' && ref.codegraphKinds?.length) {
      return ref.codegraphKinds.join(', ');
    }
    if (ref.symbol.trim()) return ref.symbol;
    if (ref.label.trim() && ref.label !== ref.path) return ref.label;
    return '';
  }
</script>

<aside class="source-tree" aria-label="Referenced files">
  <header><strong>Files</strong><span>{fileCount}</span></header>
  <div class="tree-scroll">
    {#if visibleNodes.length === 0}
      <div class="source-tree-empty app-state-fill">
        <FileCode2 class="size-4" strokeWidth={1.7} aria-hidden="true" />
        <strong>No source references</strong>
        <span>This flow has no linked files.</span>
      </div>
    {:else}
      <div class="tree" role="tree" aria-label="Source file tree" bind:this={treeRoot}>
        {#each visibleNodes as item (itemId(item))}
          {#if item.kind === 'range'}
            {@const detail = rangeDetail(item.ref)}
            <Button
              variant="ghost"
              class={cn(
                'tree-row range-row justify-start font-normal focus-visible:border-transparent focus-visible:ring-0 active:not-aria-[haspopup]:translate-y-0',
                item.ref.id === selectedRefId && 'selected',
                item.ref.status !== 'current' && 'stale',
              )}
              style="--tree-depth: {item.depth}"
              role="treeitem"
              aria-level={item.depth + 1}
              aria-selected={item.ref.id === selectedRefId}
              tabindex={itemId(item) === activeTreeItemId ? 0 : -1}
              data-tree-id={itemId(item)}
              title={`${item.file.path}:${item.ref.startLine}-${item.ref.endLine}`}
              onclick={() => onselect(item.ref.id)}
              onkeydown={(event) => handleKeydown(event, item)}
            >
              <span class="range-branch" aria-hidden="true"></span>
              <span class="range-label">L{item.ref.startLine}-{item.ref.endLine}</span>
              {#if detail}<small>{detail}</small>{/if}
            </Button>
          {:else if item.node.kind === 'folder'}
            <Button
              variant="ghost"
              class="tree-row folder-row justify-start font-normal focus-visible:border-transparent focus-visible:ring-0 active:not-aria-[haspopup]:translate-y-0"
              style="--tree-depth: {item.depth}"
              role="treeitem"
              aria-level={item.depth + 1}
              aria-expanded={isExpanded(item.node.id)}
              aria-selected="false"
              tabindex={itemId(item) === activeTreeItemId ? 0 : -1}
              data-tree-id={item.node.id}
              title={item.node.path}
              onclick={() => toggleNode(item.node.id)}
              onkeydown={(event) => handleKeydown(event, item)}
            >
              <ChevronRight class={`disclosure${isExpanded(item.node.id) ? ' expanded' : ''}`} />
              {#if isExpanded(item.node.id)}
                <FolderOpen class="tree-icon" strokeWidth={1.7} />
              {:else}
                <Folder class="tree-icon" strokeWidth={1.7} />
              {/if}
              <span>{item.node.name}</span>
            </Button>
          {:else}
            <Button
              variant="ghost"
              class={cn(
                'tree-row file-row justify-start font-normal focus-visible:border-transparent focus-visible:ring-0 active:not-aria-[haspopup]:translate-y-0',
                item.node.refs.length > 1 && item.node.id === selectedFileId && 'context',
                item.node.refs.length === 1 && item.node.id === selectedFileId && 'selected',
                item.node.refs.some((ref) => ref.status !== 'current') && 'stale',
                item.node.refs.every((ref) => ref.origin === 'codegraph') && 'related',
              )}
              style="--tree-depth: {item.depth}"
              role="treeitem"
              aria-level={item.depth + 1}
              aria-expanded={item.node.refs.length > 1 ? isExpanded(item.node.id) : undefined}
              aria-selected={item.node.refs.length === 1 && item.node.id === selectedFileId}
              tabindex={itemId(item) === activeTreeItemId ? 0 : -1}
              data-tree-id={item.node.id}
              title={fileTitle(item.node)}
              onclick={() => selectFile(item.node)}
              onkeydown={(event) => handleKeydown(event, item)}
            >
              {#if item.node.refs.length > 1}
                <ChevronRight class={`disclosure${isExpanded(item.node.id) ? ' expanded' : ''}`} />
              {:else}
                <span class="file-indent" aria-hidden="true"></span>
              {/if}
              <FileCode2 class="tree-icon" strokeWidth={1.7} />
              <span class="file-name">{item.node.name}</span>
              <small>{rangeLabel(item.node.refs)}</small>
            </Button>
          {/if}
        {/each}
      </div>
    {/if}
  </div>
</aside>

<style>
  .source-tree {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex-direction: column;
    background: color-mix(in oklab, var(--panel) 94%, var(--workspace) 6%);
    box-shadow: var(--pane-separation-shadow);
    z-index: 1;
  }

  .source-tree > header {
    position: relative;
    display: flex;
    height: 44px;
    flex: 0 0 44px;
    align-items: center;
    justify-content: space-between;
    padding: 0 11px 0 13px;
    background: var(--panel-header-surface);
    box-shadow: 0 10px 20px -23px color-mix(in oklab, var(--foreground) 24%, transparent);
    font-size: 11.5px;
  }

  .source-tree > header::after {
    position: absolute;
    right: 12px;
    bottom: 0;
    left: 12px;
    height: 1px;
    background: var(--panel-divider);
    content: '';
  }

  .source-tree > header strong {
    font-weight: 620;
  }

  .source-tree > header span {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .tree-scroll {
    display: flex;
    min-height: 0;
    flex: 1;
    overflow: auto;
    scrollbar-width: thin;
  }

  .source-tree-empty {
    width: 100%;
    min-height: 100%;
    gap: 8px;
    color: var(--muted-foreground);
    font-size: 10.5px;
  }

  .source-tree-empty > strong {
    color: var(--foreground);
    font-size: 11px;
    font-weight: 600;
  }

  .source-tree-empty > span {
    max-width: 18ch;
    line-height: 1.45;
  }

  .tree {
    display: grid;
    width: max-content;
    align-content: start;
    gap: 2px;
    min-width: 100%;
    padding: 7px;
  }

  .tree :global(.tree-row) {
    display: flex;
    width: max-content;
    min-width: 100%;
    height: 30px;
    align-items: center;
    gap: 6px;
    padding: 0 7px 0 calc(5px + var(--tree-depth) * 15px);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    color: var(--muted-foreground);
    background: transparent;
    text-align: left;
    font-size: 11.5px;
    letter-spacing: 0;
    transition:
      color var(--dur-micro) var(--ease-out),
      background-color var(--dur-micro) var(--ease-out),
      border-color var(--dur-micro) var(--ease-out);
  }

  .tree :global(.tree-row:hover) {
    color: var(--foreground);
    background: color-mix(in oklab, var(--muted) 64%, transparent);
  }

  .tree :global(.tree-row:active) {
    background: color-mix(in oklab, var(--muted) 78%, transparent);
    box-shadow: var(--inset-shadow);
  }

  .tree :global(.tree-row:disabled) {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .tree :global(.tree-row:focus-visible) {
    outline: 2px solid var(--ring);
    outline-offset: -2px;
  }

  .tree :global(.tree-row.selected) {
    border-color: color-mix(in oklab, var(--signal) 30%, var(--border));
    color: var(--foreground);
    background: color-mix(in oklab, var(--signal) 9%, var(--panel));
    box-shadow: var(--selected-shadow);
  }

  .tree :global(.tree-row.context) {
    color: var(--foreground);
    background: color-mix(in oklab, var(--muted) 42%, transparent);
  }

  .tree :global(.tree-row.stale small),
  .tree :global(.tree-row.stale .range-label) {
    color: var(--status-warning);
  }

  .tree :global(.tree-row.related small) {
    color: color-mix(in oklab, var(--muted-foreground) 82%, var(--signal) 18%);
  }

  :global(.source-tree .disclosure),
  :global(.source-tree .tree-icon) {
    width: 14px;
    height: 14px;
    flex: 0 0 14px;
  }

  :global(.source-tree .disclosure) {
    transition: transform var(--dur-micro) var(--ease-out);
  }

  :global(.source-tree .disclosure.expanded) {
    transform: rotate(90deg);
  }

  .file-indent {
    width: 14px;
    flex: 0 0 14px;
  }

  .tree :global(.folder-row span),
  .file-name {
    white-space: nowrap;
  }

  .tree :global(.folder-row span) {
    font-weight: 550;
  }

  .file-name {
    flex: 0 0 auto;
    color: inherit;
    font-family: var(--app-font-mono);
    font-size: 10.8px;
  }

  .tree :global(.file-row small) {
    flex: 0 0 auto;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .tree :global(.range-row) {
    gap: 7px;
    padding-left: calc(10px + var(--tree-depth) * 15px);
  }

  .range-branch {
    width: 10px;
    height: 9px;
    flex: 0 0 10px;
    align-self: start;
    margin-top: 3px;
    border-bottom: 1px solid var(--border);
    border-left: 1px solid var(--border);
    border-bottom-left-radius: 4px;
  }

  .range-label {
    flex: 0 0 auto;
    color: inherit;
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .tree :global(.range-row small) {
    flex: 0 0 auto;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
    white-space: nowrap;
  }
</style>
