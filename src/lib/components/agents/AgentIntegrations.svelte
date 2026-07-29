<script lang="ts">
  import { onMount } from 'svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { agentSetupStatus, type AgentSetupStatus } from '$lib/adapters/agents';
  import { inspectorStore } from '$stores/inspector.svelte';

  let { variant = 'onboarding' }: { variant?: 'onboarding' | 'settings' } = $props();

  const store = inspectorStore;
  let selectedAgentIds = $state(new Set<string>());
  let selectionPrimed = $state(false);
  let userToggled = $state(new Set<string>());

  // Detect on every mount: agents installed after the app launched must be
  // pickable without restarting the desktop.
  onMount(() => {
    void store.detectAgents();
  });

  // Prime the selection once detection lands: every detected agent that still
  // needs an install or an update is opted in by default. Choices the user
  // toggled explicitly are kept as-is across re-primes.
  $effect(() => {
    if (selectionPrimed || store.agentIntegrationsStatus !== 'ready') return;
    selectionPrimed = true;
    const defaults = store.agentIntegrations
      .filter(
        (agent) =>
          agent.detected && agentSetupStatus(agent) !== 'installed' && !userToggled.has(agent.id),
      )
      .map((agent) => agent.id);
    selectedAgentIds = new Set([...selectedAgentIds, ...defaults]);
  });

  const detectedCount = $derived(
    store.agentIntegrations.filter((agent) => agent.detected).length,
  );
  const actionableCount = $derived(
    store.agentIntegrations.filter(
      (agent) => agent.detected && agentSetupStatus(agent) !== 'installed',
    ).length,
  );
  const shouldRender = $derived(
    store.agentIntegrationsStatus === 'ready' &&
      detectedCount > 0 &&
      (variant === 'settings' || (!store.agentSetupDismissed && actionableCount > 0)),
  );
  const pendingSelection = $derived(
    [...selectedAgentIds].filter((id) => {
      const agent = store.agentIntegrations.find((entry) => entry.id === id);
      return agent && agentSetupStatus(agent) !== 'installed';
    }),
  );

  function statusLabel(status: AgentSetupStatus): string {
    if (status === 'installed') return 'Installed';
    if (status === 'update-available') return 'Update available';
    if (status === 'not-installed') return 'Not installed';
    return 'Not detected';
  }

  function setSelected(agentId: string, checked: boolean) {
    userToggled = new Set(userToggled).add(agentId);
    selectedAgentIds = new Set(selectedAgentIds);
    if (checked) selectedAgentIds.add(agentId);
    else selectedAgentIds.delete(agentId);
  }

  async function installSelected() {
    await store.installAgents(pendingSelection);
  }

  async function recheck() {
    await store.detectAgents();
    // Re-prime the default selection from the fresh detection result.
    selectionPrimed = false;
  }
</script>

{#if shouldRender || store.agentInstallError || store.agentIntegrationsStatus === 'error'}
  <div class="agent-integrations" data-variant={variant}>
    {#if shouldRender}
      {#if variant === 'onboarding'}
        <div class="agent-setup-copy">
          <strong>Connect your coding agents</strong>
          <span
            >Install VibeInspector's inspector skills and MCP server into the agents on this
            Mac.</span
          >
        </div>
      {/if}
      <div class="agent-rows">
        {#each store.agentIntegrations as agent (agent.id)}
          {@const status = agentSetupStatus(agent)}
          <label class="agent-row" class:disabled={!agent.detected}>
            <Checkbox
              checked={selectedAgentIds.has(agent.id)}
              disabled={!agent.detected || status === 'installed'}
              onCheckedChange={(checked) => setSelected(agent.id, checked)}
              aria-label={`Install VibeInspector skills and MCP for ${agent.name}`}
            />
            <span class="agent-copy">
              <strong>{agent.name}</strong>
              <span class="agent-path" title={agent.detectionEvidence.join(', ') || agent.skillsDir}>
                {agent.detected ? agent.skillsDir : 'No local installation found'}
              </span>
            </span>
            <Badge variant="outline" class={`agent-status agent-status-${status}`}
              >{statusLabel(status)}</Badge
            >
          </label>
        {/each}
      </div>

      <div class="agent-footer">
        <span class="agent-note">Installs inspector skills and the local MCP server config.</span>
        <div class="agent-actions">
          {#if variant === 'onboarding'}
            <button type="button" class="agent-link" onclick={() => store.dismissAgentSetup()}>
              Skip for now
            </button>
          {:else}
            <button
              type="button"
              class="agent-link"
              disabled={store.agentDetecting}
              onclick={recheck}
            >
              Re-check
            </button>
          {/if}
          <Button
            size="sm"
            disabled={pendingSelection.length === 0 || store.agentInstallUpdating}
            onclick={installSelected}
          >
            {store.agentInstallUpdating ? 'Installing…' : `Install selected`}
          </Button>
        </div>
      </div>
    {/if}

    {#if store.agentInstallError || store.agentIntegrationsStatus === 'error'}
      <p class="agent-error" role="alert">
        {store.agentInstallError || 'Agent detection failed. Reopen this view to retry.'}
      </p>
    {/if}
  </div>
{/if}

<style>
  .agent-integrations {
    display: grid;
    gap: 10px;
  }

  .agent-setup-copy {
    display: grid;
    gap: 3px;
  }

  .agent-setup-copy strong {
    font-size: 13px;
    font-weight: 620;
  }

  .agent-setup-copy span {
    color: var(--muted-foreground);
    font-size: 12px;
    line-height: 1.5;
  }

  .agent-rows {
    display: grid;
    gap: 6px;
  }

  .agent-row {
    display: grid;
    min-width: 0;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in oklab, var(--muted) 38%, transparent);
    box-shadow: var(--item-shadow);
    cursor: pointer;
  }

  .agent-row.disabled {
    cursor: default;
    opacity: 0.58;
  }

  .agent-copy {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .agent-copy strong {
    font-size: 12.5px;
    font-weight: 610;
  }

  .agent-path {
    overflow: hidden;
    color: var(--muted-foreground);
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .agent-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .agent-note {
    color: var(--muted-foreground);
    font-size: 11px;
    line-height: 1.45;
  }

  .agent-actions {
    display: flex;
    flex: 0 0 auto;
    align-items: center;
    gap: 12px;
  }

  .agent-link {
    padding: 0;
    border: 0;
    color: var(--muted-foreground);
    background: none;
    font-size: 11.5px;
    text-decoration: underline;
    text-underline-offset: 3px;
    cursor: pointer;
    transition: color var(--dur-micro) var(--ease-out);
  }

  .agent-link:hover {
    color: var(--foreground);
  }

  .agent-link:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .agent-error {
    margin: 0;
    color: var(--status-error);
    font-size: 11px;
  }

  :global(.agent-status) {
    font-family: var(--app-font-mono);
    font-size: 9.5px;
  }

  :global(.agent-status-installed) {
    color: var(--status-success);
  }

  :global(.agent-status-update-available) {
    color: var(--status-warning);
  }
</style>
