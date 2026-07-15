<script lang="ts">
  import { Check, Copy, FileCode2, LockKeyhole, TriangleAlert } from '@lucide/svelte';
  import { onDestroy, onMount } from 'svelte';
  import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
  import JsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
  import type { SourceRef } from '$domain/entities';
  import {
    isEditorWriteShortcut,
    lineNumberMinChars,
    loadEditorLanguage,
    readonlyEditorOptions,
    shortContentHash,
  } from '$lib/adapters/editor';

  let {
    file,
    code,
    codeStartLine = 1,
    readStatus = 'current',
    theme = 'dark',
    embedded = false,
  } = $props<{
    file: SourceRef;
    code: string;
    codeStartLine?: number;
    readStatus?: 'current' | 'stale' | 'unreadable';
    theme?: 'dark' | 'light';
    embedded?: boolean;
  }>();
  let container = $state<HTMLDivElement>();
  let cardElement = $state<HTMLElement>();
  let editor: import('monaco-editor').editor.IStandaloneCodeEditor | undefined;
  let monaco: typeof import('monaco-editor/esm/vs/editor/editor.api') | undefined;
  let loading = $state(true);
  let loadError = $state('');
  let copied = $state(false);
  let decorationIds: string[] = [];
  let languageRequest = 0;

  const lineCount = $derived(code.split('\n').length);
  const displayedContentHash = $derived(shortContentHash(file.contentHash));

  function editorTheme(currentTheme: 'dark' | 'light') {
    return currentTheme === 'dark' ? 'vibe-inspector-dark' : 'vibe-inspector-light';
  }

  function editorToken(name: string): string {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    if (!value) throw new Error(`Missing editor theme token: ${name}`);
    return value;
  }

  function defineEditorTheme(editorApi: typeof monaco, currentTheme: 'dark' | 'light') {
    if (!editorApi) return;
    editorApi.editor.defineTheme(editorTheme(currentTheme), {
      base: currentTheme === 'dark' ? 'vs-dark' : 'vs',
      inherit: true,
      rules: [],
      colors: {
        'editor.background': editorToken('--editor-background'),
        'editor.foreground': editorToken('--editor-foreground'),
        'editorLineNumber.foreground': editorToken('--editor-line-number'),
        'editorLineNumber.activeForeground': editorToken('--editor-line-number-active'),
        'editor.lineHighlightBackground': editorToken('--editor-line-highlight'),
        'editor.selectionBackground': editorToken('--editor-selection'),
        'editor.inactiveSelectionBackground': editorToken('--editor-selection-inactive'),
        'editorGutter.background': editorToken('--editor-background'),
      },
    });
  }

  function highlightReference() {
    if (!editor || !monaco) return;
    const localStart = Math.max(1, file.startLine - codeStartLine + 1);
    const localEnd = Math.max(localStart, file.endLine - codeStartLine + 1);
    decorationIds = editor.deltaDecorations(decorationIds, [
      {
        range: new monaco.Range(localStart, 1, localEnd, 1),
        options: {
          isWholeLine: true,
          className: 'vi-line-highlight',
          overviewRuler: {
            color: editorToken('--editor-reference'),
            position: monaco.editor.OverviewRulerLane.Full,
          },
        },
      },
    ]);
    editor.revealLineInCenter(localStart);
    editor.setPosition({ lineNumber: localStart, column: 1 });
  }

  async function copyPath() {
    if (typeof navigator === 'undefined') return;
    await navigator.clipboard?.writeText(`${file.path}:${file.startLine}`);
    copied = true;
    window.setTimeout(() => (copied = false), 1600);
  }

  function blockWriteShortcut(event: KeyboardEvent) {
    if (!cardElement?.contains(event.target as Node) || !isEditorWriteShortcut(event)) return;
    event.preventDefault();
    event.stopPropagation();
  }

  function blockPaste(event: ClipboardEvent) {
    if (!cardElement?.contains(event.target as Node)) return;
    event.preventDefault();
    event.stopPropagation();
  }

  async function updateEditorLanguage(path: string) {
    const request = ++languageRequest;
    const language = await loadEditorLanguage(path);
    if (request !== languageRequest || !editor || !monaco) return;
    const model = editor.getModel();
    if (model) monaco.editor.setModelLanguage(model, language);
  }

  onMount(async () => {
    try {
      const workerGlobal = globalThis as typeof globalThis & {
        MonacoEnvironment?: { getWorker: (_moduleId: string, label: string) => Worker };
      };
      workerGlobal.MonacoEnvironment = {
        getWorker: (_moduleId, label) => (label === 'json' ? new JsonWorker() : new EditorWorker()),
      };
      await loadEditorLanguage(file.path);
      monaco = await import('monaco-editor/esm/vs/editor/editor.api');
      defineEditorTheme(monaco, theme);
      if (!container) return;
      editor = monaco.editor.create(
        container,
        readonlyEditorOptions({
          value: code,
          path: file.path,
          theme: editorTheme(theme),
          codeStartLine,
        }),
      );
      loading = false;
      highlightReference();
    } catch (error) {
      loadError = error instanceof Error ? error.message : 'Monaco could not be loaded';
      loading = false;
    }
  });

  $effect(() => {
    const nextCode = code;
    const nextPath = file.path;
    const nextTheme = theme;
    const nextStartLine = codeStartLine;
    if (!editor || !monaco) return;
    const model = editor.getModel();
    if (model && model.getValue() !== nextCode) model.setValue(nextCode);
    editor.updateOptions({
      lineNumbers: (lineNumber) => String(lineNumber + nextStartLine - 1),
      lineNumbersMinChars: lineNumberMinChars(nextCode, nextStartLine),
    });
    void updateEditorLanguage(nextPath);
    defineEditorTheme(monaco, nextTheme);
    monaco.editor.setTheme(editorTheme(nextTheme));
    highlightReference();
  });

  onDestroy(() => {
    languageRequest += 1;
    editor?.dispose();
  });
</script>

<svelte:window onkeydown={blockWriteShortcut} onpaste={blockPaste} />

<section
  bind:this={cardElement}
  class:embedded
  class="code-card"
  aria-label="Read-only source viewer"
>
  <header class="code-header">
    <div class="file-heading">
      <span class="file-icon"><FileCode2 aria-hidden="true" /></span>
      <div>
        <strong>{file.path.split('/').at(-1)}</strong>
        <span>{file.path}</span>
      </div>
    </div>
    <div class="code-meta">
      <span class="readonly-lock"><LockKeyhole aria-hidden="true" /> read only</span>
      <span>{lineCount} lines</span>
      <button class="copy-path" onclick={copyPath} aria-label="Copy source path">
        {#if copied}<Check aria-hidden="true" />{:else}<Copy aria-hidden="true" />{/if}
        {copied ? 'Copied' : 'Copy path'}
      </button>
    </div>
  </header>
  <div class="editor-shell">
    <div bind:this={container} class="editor-container"></div>
    {#if loading}
      <div class="editor-state"><span class="loader-ring"></span> Loading editor</div>
    {:else if loadError}
      <div class="editor-state error">
        <TriangleAlert aria-hidden="true" />
        {loadError}
      </div>
    {/if}
  </div>
  <footer class="code-footer">
    <span><span class="range-marker"></span> referenced lines {file.startLine}-{file.endLine}</span>
    <span
      class:stale={readStatus === 'stale'}
      class="content-hash"
      title={`Content hash: ${file.contentHash}`}
      >{readStatus === 'stale' ? 'stale · ' : ''}hash {displayedContentHash}</span
    >
  </footer>
</section>

<style>
  .code-card {
    display: grid;
    height: 100%;
    min-width: 0;
    min-height: 220px;
    grid-template-rows: auto minmax(0, 1fr) auto;
    overflow: hidden;
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-lg);
    background: var(--card);
    box-shadow: var(--panel-shadow);
    container-type: inline-size;
  }

  .code-card.embedded {
    border: 0;
    border-radius: 0;
    box-shadow: none;
  }

  .code-header,
  .code-footer {
    display: flex;
    width: 100%;
    min-width: 0;
    min-height: 44px;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 0 13px;
    overflow: hidden;
    border-bottom: 1px solid var(--border);
  }

  .code-footer {
    min-height: 28px;
    border-top: 1px solid var(--border);
    border-bottom: 0;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .code-footer > span {
    display: inline-flex;
    min-width: 0;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .code-footer > span:first-child {
    flex: 1 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .range-marker {
    width: 6px;
    height: 6px;
    border-radius: 2px;
    background: var(--signal);
  }

  .content-hash.stale {
    color: var(--status-warning);
  }

  .content-hash {
    max-width: 46%;
    flex: 0 1 auto;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file-heading,
  .code-meta,
  .readonly-lock,
  .copy-path {
    display: flex;
    align-items: center;
  }

  .file-heading {
    min-width: 0;
    flex: 1 1 auto;
    gap: 9px;
    overflow: hidden;
  }

  .file-icon {
    display: grid;
    width: 25px;
    height: 25px;
    flex: 0 0 25px;
    place-items: center;
    border-radius: 6px;
    color: var(--muted-foreground);
    background: var(--muted);
  }

  .file-icon :global(svg) {
    width: 13px;
  }

  .file-heading > div {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .file-heading strong,
  .file-heading span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-heading strong {
    color: var(--foreground);
    font-family: var(--app-font-mono);
    font-size: 12px;
  }

  .file-heading > div > span {
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .code-meta {
    min-width: 0;
    flex: 0 0 auto;
    gap: 11px;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
  }

  .readonly-lock {
    gap: 4px;
    color: var(--status-success);
  }

  .readonly-lock :global(svg),
  .copy-path :global(svg) {
    width: 11px;
  }

  .copy-path {
    gap: 5px;
    padding: 4px 0 4px 7px;
    border: 0;
    border-left: 1px solid var(--border);
    color: var(--muted-foreground);
    background: transparent;
    cursor: pointer;
    white-space: nowrap;
  }

  .copy-path:hover {
    color: var(--foreground);
  }

  .editor-shell {
    position: relative;
    min-height: 0;
  }

  .editor-container {
    position: absolute;
    inset: 0;
  }

  .editor-state {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    color: var(--muted-foreground);
    background: var(--card);
    font-family: var(--app-font-mono);
    font-size: 11px;
  }

  .editor-state.error {
    color: var(--destructive);
  }

  .editor-state :global(svg) {
    width: 15px;
  }

  .loader-ring {
    width: 14px;
    height: 14px;
    border: 2px solid color-mix(in oklab, var(--signal) 20%, transparent);
    border-top-color: var(--signal);
    border-radius: 999px;
    animation: spin 700ms linear infinite;
  }

  :global(.vi-line-highlight) {
    background: color-mix(in srgb, var(--editor-reference) 12%, transparent);
    border-left: 2px solid var(--editor-reference);
  }

  :global(.monaco-editor),
  :global(.monaco-editor *) {
    -webkit-font-smoothing: auto;
  }

  @container (max-width: 460px) {
    .code-header,
    .code-footer {
      gap: 8px;
      padding-right: 10px;
      padding-left: 10px;
    }

    .code-meta {
      gap: 8px;
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
