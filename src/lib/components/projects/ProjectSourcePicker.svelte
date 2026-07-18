<script lang="ts">
  import {
    ChevronLeft,
    ChevronRight,
    FolderGit2,
    FolderPlus,
    Folders,
    History,
    RefreshCw,
    Search,
    TriangleAlert,
  } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import {
    chooseProjectDirectory,
    discoverProjectDirectory,
    discoveryErrorMessage,
    scanCodexHistory,
    type ProjectCandidate,
    type ProjectDiscoveryMode,
    type ProjectDiscoveryResult,
  } from '$lib/adapters/projects';
  import { inspectorStore } from '$stores/inspector.svelte';

  type PickerStep = 'choose' | 'scanning' | 'review' | 'registering' | 'empty' | 'error';
  type ErrorKind = 'discovery' | 'registration';

  let {
    onComplete,
    onStepChange,
  }: {
    onComplete?: () => void;
    onStepChange?: (step: PickerStep) => void;
  } = $props();

  const store = inspectorStore;
  let step = $state<PickerStep>('choose');
  let result = $state<ProjectDiscoveryResult>();
  let selectedRoots = $state(new Set<string>());
  let errorMessage = $state('');
  let errorKind = $state<ErrorKind>('discovery');
  let historyEvidence = $state(
    new Map<string, { conversationIds: string[]; parserVersion: string }>(),
  );
  let reviewSource = $state<'manual' | 'codex-history'>('manual');

  const selectedCandidates = $derived(
    result?.candidates.filter((candidate) => selectedRoots.has(candidate.rootPath)) ?? [],
  );
  const allCandidatesSelected = $derived(
    (result?.candidates.length ?? 0) > 0 &&
      selectedCandidates.length === (result?.candidates.length ?? 0),
  );
  const selectionIsMixed = $derived(
    selectedCandidates.length > 0 && selectedCandidates.length < (result?.candidates.length ?? 0),
  );
  const reviewViewportHeight = $derived(Math.min((result?.candidates.length ?? 0) * 58 + 8, 336));

  $effect(() => {
    onStepChange?.(step);
  });

  async function selectAndScan(mode: ProjectDiscoveryMode) {
    errorMessage = '';
    errorKind = 'discovery';
    step = 'scanning';

    try {
      const rootPath = await chooseProjectDirectory(mode);
      if (!rootPath) {
        step = 'choose';
        return;
      }
      result = await discoverProjectDirectory(rootPath, mode);
      reviewSource = 'manual';
      historyEvidence = new Map();
      selectedRoots = new Set(result.candidates.map((candidate) => candidate.rootPath));
      step = result.candidates.length === 0 ? 'empty' : 'review';
    } catch (error) {
      errorMessage = discoveryErrorMessage(error);
      step = 'error';
    }
  }

  async function scanHistory() {
    errorMessage = '';
    errorKind = 'discovery';
    step = 'scanning';
    try {
      const history = await scanCodexHistory();
      result = {
        selectedRoot: 'Local Codex history',
        mode: 'collection',
        candidates: history.proposals,
        visitedDirectories: history.scannedThreads,
        truncated: history.truncated,
      };
      reviewSource = 'codex-history';
      historyEvidence = new Map(
        history.proposals.map((proposal) => [
          proposal.rootPath,
          {
            conversationIds: proposal.conversationIds,
            parserVersion: proposal.parserVersion,
          },
        ]),
      );
      selectedRoots = new Set();
      step = result.candidates.length === 0 ? 'empty' : 'review';
    } catch (error) {
      errorMessage = discoveryErrorMessage(error);
      step = 'error';
    }
  }

  function setSelected(candidate: ProjectCandidate, checked: boolean) {
    selectedRoots = new Set(selectedRoots);
    if (checked) selectedRoots.add(candidate.rootPath);
    else selectedRoots.delete(candidate.rootPath);
  }

  function setAllSelected(checked: boolean) {
    selectedRoots = checked
      ? new Set(result?.candidates.map((candidate) => candidate.rootPath) ?? [])
      : new Set();
  }

  function projectKindLabel(kind: ProjectCandidate['kind']) {
    if (kind === 'monorepo') return 'Monorepo';
    if (kind === 'repository') return 'Repository';
    return 'Project';
  }

  function conversationLabel(count: number) {
    return `${count} Codex ${count === 1 ? 'conversation' : 'conversations'}`;
  }

  function markerLabel(markers: string[]) {
    const visibleMarkers = markers.slice(0, 2);
    const remaining = markers.length - visibleMarkers.length;
    return `${visibleMarkers.join(', ')}${remaining > 0 ? ` +${remaining}` : ''}`;
  }

  async function registerProjects() {
    errorMessage = '';
    errorKind = 'registration';
    step = 'registering';
    try {
      await store.registerProjectCandidates(
        selectedCandidates,
        selectedCandidates.map((candidate) => {
          const history = historyEvidence.get(candidate.rootPath);
          return {
            rootPath: candidate.rootPath,
            source: reviewSource,
            conversationIds: history?.conversationIds ?? [],
            parserVersion: history?.parserVersion,
          };
        }),
      );
      onComplete?.();
    } catch (error) {
      errorMessage = discoveryErrorMessage(error);
      step = 'error';
    }
  }

  function reset() {
    result = undefined;
    selectedRoots = new Set();
    errorMessage = '';
    errorKind = 'discovery';
    historyEvidence = new Map();
    reviewSource = 'manual';
    step = 'choose';
  }
</script>

{#if step === 'choose'}
  <div class="choice-list">
    <button class="app-focus-ring" onclick={scanHistory}>
      <span class="choice-icon"><History strokeWidth={1.7} /></span>
      <span class="choice-copy">
        <strong>Codex history</strong>
        <span>Find repositories referenced by your local Codex conversations.</span>
      </span>
      <ChevronRight class="size-4 text-muted-foreground" strokeWidth={1.7} />
    </button>
    <button class="app-focus-ring" onclick={() => selectAndScan('collection')}>
      <span class="choice-icon"><Folders strokeWidth={1.7} /></span>
      <span class="choice-copy">
        <strong>Projects folder</strong>
        <span>Scan a parent folder for repository and workspace roots.</span>
      </span>
      <ChevronRight class="size-4 text-muted-foreground" strokeWidth={1.7} />
    </button>
    <button class="app-focus-ring" onclick={() => selectAndScan('single-project')}>
      <span class="choice-icon"><FolderGit2 strokeWidth={1.7} /></span>
      <span class="choice-copy">
        <strong>Single project</strong>
        <span>Add one repository or workspace from its root folder.</span>
      </span>
      <ChevronRight class="size-4 text-muted-foreground" strokeWidth={1.7} />
    </button>
  </div>

  <p class="boundary-note">
    Codex discovery reads project paths and conversation IDs, never message content.
  </p>
{:else if step === 'scanning'}
  <div class="state-view app-state-fill" aria-live="polite">
    <span class="state-icon scanning"><Search strokeWidth={1.7} /></span>
    <strong>Looking for project roots</strong>
    <span>Checking repository and workspace markers.</span>
  </div>
{:else if step === 'registering'}
  <div class="state-view app-state-fill" aria-live="polite" aria-busy="true">
    <span class="state-icon scanning"><FolderPlus strokeWidth={1.7} /></span>
    <strong>Adding projects</strong>
    <span>Saving the selected repositories to your local workspace.</span>
  </div>
{:else if step === 'error'}
  <div class="state-view app-state-fill" role="alert">
    <span class="state-icon error"><TriangleAlert strokeWidth={1.7} /></span>
    <strong
      >{errorKind === 'registration' ? 'Could not add projects' : 'Could not scan projects'}</strong
    >
    <span>{errorMessage}</span>
    {#if errorKind === 'registration'}
      <div class="error-actions">
        <Button size="sm" onclick={registerProjects}>
          <RefreshCw data-icon="inline-start" />Retry
        </Button>
        <Button variant="ghost" size="sm" onclick={() => (step = 'review')}>Review selection</Button
        >
      </div>
    {:else}
      <Button variant="outline" size="sm" onclick={reset}>Try another source</Button>
    {/if}
  </div>
{:else if result}
  {#if result.candidates.length === 0}
    <div class="state-view compact app-state-fill">
      <span class="state-icon"><Search strokeWidth={1.7} /></span>
      <strong>No projects found</strong>
      <span>Choose a different folder or add one project directly.</span>
      <Button variant="outline" size="sm" onclick={reset}>Try another source</Button>
    </div>
  {:else}
    <div class="review-context">
      <span class="review-source" title={result.selectedRoot}>
        {#if reviewSource === 'codex-history'}
          <History strokeWidth={1.7} />
        {:else}
          <Folders strokeWidth={1.7} />
        {/if}
        <span>{result.selectedRoot}</span>
      </span>
      <span
        >{result.candidates.length} {result.candidates.length === 1 ? 'project' : 'projects'}</span
      >
    </div>

    <div class="selection-bar">
      <label>
        <Checkbox
          checked={allCandidatesSelected}
          indeterminate={selectionIsMixed}
          onCheckedChange={setAllSelected}
          aria-label="Select all detected projects"
        />
        <span>Select all</span>
      </label>
      <span>{selectedCandidates.length} selected</span>
    </div>

    <ScrollArea class="candidate-scroll" style={`height: min(${reviewViewportHeight}px, 42vh);`}>
      <div class="candidate-list">
        {#each result.candidates as candidate (candidate.rootPath)}
          <label class:selected={selectedRoots.has(candidate.rootPath)}>
            <Checkbox
              checked={selectedRoots.has(candidate.rootPath)}
              onCheckedChange={(checked) => setSelected(candidate, checked)}
              aria-label={`Add ${candidate.name}`}
            />
            <span class="candidate-copy">
              <span class="candidate-heading">
                <strong>{candidate.name}</strong>
                <span>{projectKindLabel(candidate.kind)}</span>
              </span>
              <span class="candidate-meta">
                <span class="candidate-path" title={candidate.rootPath}>
                  {reviewSource === 'codex-history' ? candidate.rootPath : candidate.relativePath}
                </span>
                <small>
                  {#if historyEvidence.has(candidate.rootPath)}
                    {conversationLabel(
                      historyEvidence.get(candidate.rootPath)?.conversationIds.length ?? 0,
                    )}
                  {:else}
                    {markerLabel(candidate.markers)}
                  {/if}
                </small>
              </span>
            </span>
          </label>
        {/each}
      </div>
    </ScrollArea>

    {#if result.truncated}
      <p class="scan-warning">
        Scan limit reached. Choose a smaller parent folder to find the remaining projects.
      </p>
    {/if}

    <div class="review-actions">
      <Button variant="ghost" size="sm" onclick={reset}
        ><ChevronLeft data-icon="inline-start" />Back</Button
      >
      <Button size="sm" disabled={selectedCandidates.length === 0} onclick={registerProjects}>
        Add {selectedCandidates.length}
        {selectedCandidates.length === 1 ? 'project' : 'projects'}
      </Button>
    </div>
  {/if}
{/if}

<style>
  .choice-list {
    display: grid;
    gap: 8px;
  }

  .choice-list > button {
    display: grid;
    min-height: 72px;
    grid-template-columns: 38px minmax(0, 1fr) 30px;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid color-mix(in oklab, var(--panel-border) 76%, transparent);
    border-radius: var(--radius-xl);
    color: var(--foreground);
    background: color-mix(in oklab, var(--card) 58%, transparent);
    box-shadow: var(--item-shadow);
    text-align: left;
    transition: background-color var(--dur-micro) var(--ease-out);
  }

  .choice-list > button:hover {
    background: color-mix(in oklab, var(--card) 86%, transparent);
  }

  .choice-list > button:active {
    background: color-mix(in oklab, var(--card) 96%, transparent);
  }

  .choice-list > button > :global(svg) {
    width: 14px;
    height: 14px;
    justify-self: center;
    padding: 7px;
    border-radius: 999px;
    box-sizing: content-box;
    background: color-mix(in oklab, var(--muted) 56%, transparent);
  }

  .choice-icon,
  .state-icon {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    color: var(--muted-foreground);
    background: var(--muted);
  }

  .choice-icon :global(svg),
  .state-icon :global(svg) {
    width: 16px;
  }

  .choice-copy,
  .candidate-copy {
    display: grid;
    min-width: 0;
  }

  .choice-copy {
    gap: 3px;
  }

  .choice-copy strong,
  .state-view strong {
    font-size: 13px;
    font-weight: 620;
  }

  .choice-copy span,
  .boundary-note,
  .review-context,
  .selection-bar,
  .state-view > span:not(.state-icon) {
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.5;
  }

  .boundary-note {
    max-width: 62ch;
    margin: 14px 12px 0;
    font-size: 11.5px;
  }

  .state-view {
    min-height: var(--state-min-height);
    gap: 8px;
    text-align: center;
  }

  .state-view.compact {
    min-height: 220px;
  }

  .state-view > span:not(.state-icon) {
    max-width: 42ch;
  }

  .error-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 4px;
  }

  .state-icon.scanning :global(svg) {
    animation: scan-pulse var(--dur-pulse) var(--ease-in-out) infinite alternate;
  }

  @media (prefers-reduced-motion: reduce) {
    .state-icon.scanning :global(svg) {
      animation: none;
    }
  }

  .state-icon.error {
    color: var(--destructive);
  }

  .review-context,
  .selection-bar {
    display: flex;
    min-width: 0;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .review-context {
    min-height: 34px;
    padding: 0 4px 10px;
    border-bottom: 1px solid var(--border);
  }

  .review-source {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 7px;
  }

  .review-source :global(svg) {
    width: 14px;
    flex: 0 0 auto;
    color: var(--muted-foreground);
  }

  .review-source > span {
    overflow: hidden;
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .review-context > span:last-child,
  .selection-bar > span {
    flex: 0 0 auto;
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .selection-bar {
    min-height: 38px;
    padding: 0 4px;
    border-bottom: 1px solid var(--border);
  }

  .selection-bar label {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--foreground);
    cursor: pointer;
  }

  .candidate-list {
    display: grid;
    gap: 8px;
    padding: 8px 4px;
  }

  .candidate-list label {
    display: grid;
    min-width: 0;
    grid-template-columns: 20px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    min-height: 58px;
    padding: 8px 12px;
    border: 1px solid color-mix(in oklab, var(--panel-border) 76%, transparent);
    border-radius: var(--radius-xl);
    color: var(--foreground);
    background: color-mix(in oklab, var(--card) 58%, transparent);
    box-shadow: var(--item-shadow);
    cursor: pointer;
    transition: background-color var(--dur-micro) var(--ease-out);
  }

  .candidate-list label:hover {
    background: color-mix(in oklab, var(--card) 86%, transparent);
  }

  .candidate-list label:active {
    background: color-mix(in oklab, var(--card) 96%, transparent);
  }

  .candidate-list label.selected {
    border-color: color-mix(in oklab, var(--signal) 34%, var(--panel-border));
    background: color-mix(in oklab, var(--signal) 7%, var(--card));
    box-shadow: var(--selected-shadow);
  }

  .candidate-list label:focus-within {
    border-color: var(--ring);
    box-shadow:
      var(--item-shadow),
      0 0 0 2px color-mix(in oklab, var(--ring) 22%, transparent);
  }

  .candidate-copy {
    gap: 5px;
  }

  .candidate-heading,
  .candidate-meta {
    display: grid;
    min-width: 0;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 16px;
  }

  .candidate-heading strong {
    overflow: hidden;
    font-size: 13px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .candidate-heading > span,
  .candidate-path,
  .candidate-meta small {
    overflow: hidden;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .candidate-heading > span,
  .candidate-meta small {
    font-size: 10px;
  }

  .candidate-path {
    font-size: 10.5px;
  }

  .candidate-meta small {
    text-align: right;
  }

  .scan-warning {
    margin: 8px 0 0;
    color: var(--status-warning);
    font-size: 11px;
    line-height: 1.45;
  }

  .review-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  @keyframes scan-pulse {
    from {
      opacity: 0.45;
      transform: scale(0.92);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }
</style>
