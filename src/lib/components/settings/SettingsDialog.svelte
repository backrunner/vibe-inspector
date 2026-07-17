<script lang="ts">
  import {
    CheckCircle2,
    CircleAlert,
    Keyboard,
    Moon,
    RefreshCw,
    ServerCog,
    Sun,
  } from '@lucide/svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import { formatRuntimeTime } from '$lib/adapters/runtime';
  import { isTauriRuntime } from '$lib/adapters/tauri';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;

  function healthy(status: string | undefined): boolean {
    return status === 'online' || status === 'desktop';
  }
</script>

<Dialog.Root bind:open={store.settingsOpen}>
  <Dialog.Content class="settings-dialog gap-0 overflow-hidden p-0 sm:max-w-[620px]">
    <Dialog.Header class="border-b px-5 py-4 text-left">
      <Dialog.Title class="text-[15px]">Settings</Dialog.Title>
      <Dialog.Description class="text-[12px]">
        Appearance and local background services
      </Dialog.Description>
    </Dialog.Header>

    <div class="settings-body">
      <section class="settings-section">
        <div class="section-copy">
          <strong>Appearance</strong>
          <span>Use a Zinc theme throughout the desktop workspace.</span>
        </div>
        <div class="segmented-control" aria-label="Application theme">
          <Button
            size="sm"
            variant="ghost"
            class={`theme-option${store.theme === 'light' ? ' selected' : ''}`}
            aria-pressed={store.theme === 'light'}
            onclick={() => store.setTheme('light')}><Sun data-icon="inline-start" />Light</Button
          >
          <Button
            size="sm"
            variant="ghost"
            class={`theme-option${store.theme === 'dark' ? ' selected' : ''}`}
            aria-pressed={store.theme === 'dark'}
            onclick={() => store.setTheme('dark')}><Moon data-icon="inline-start" />Dark</Button
          >
        </div>
      </section>

      <section class="service-section">
        <header>
          <div class="section-copy">
            <strong>Background services</strong>
            <span>The independent tray process owns MCP availability.</span>
          </div>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Refresh service health"
            title="Refresh service health"
            disabled={store.runtimeHealthStatus === 'loading'}
            onclick={() => void store.refreshRuntimeHealth()}><RefreshCw /></Button
          >
        </header>

        {#if store.runtimeHealthStatus === 'loading' && !store.runtimeHealth}
          <div class="health-skeleton" aria-label="Loading service health">
            <Skeleton class="h-12 w-full rounded-lg" />
            <Skeleton class="h-12 w-full rounded-lg" />
          </div>
        {:else if store.runtimeHealth}
          <div class="service-rows">
            <div class="service-row">
              <span class:healthy={healthy(store.runtimeHealth.tray)} class="service-icon">
                {#if healthy(store.runtimeHealth.tray)}<CheckCircle2 />{:else}<CircleAlert />{/if}
              </span>
              <div>
                <strong>Menu bar service</strong><span
                  >PID {store.runtimeHealth.pid ?? 'unavailable'}</span
                >
              </div>
              <Badge variant="outline">{store.runtimeHealth.tray}</Badge>
            </div>
            <div class="service-row">
              <span class:healthy={healthy(store.runtimeHealth.mcp)} class="service-icon">
                {#if healthy(store.runtimeHealth.mcp)}<CheckCircle2 />{:else}<CircleAlert />{/if}
              </span>
              <div>
                <strong>MCP server</strong>
                <span title={store.runtimeHealth.endpoint}
                  >{store.runtimeHealth.endpoint ?? 'No local endpoint'}</span
                >
              </div>
              <Badge variant="outline">{store.runtimeHealth.mcp}</Badge>
            </div>
          </div>
          <div class="health-meta">
            <span>Started {formatRuntimeTime(store.runtimeHealth.startedAt)}</span>
            <span>Checked {formatRuntimeTime(store.runtimeHealth.checkedAt)}</span>
          </div>
        {/if}

        {#if store.runtimeHealthError || store.runtimeHealth?.error}
          <div class="settings-error" role="alert">
            <CircleAlert />
            <span>{store.runtimeHealthError || store.runtimeHealth?.error}</span>
          </div>
        {/if}

        <label class:disabled={!isTauriRuntime()} class="login-setting">
          <Checkbox
            checked={store.runtimeHealth?.loginItemEnabled ?? false}
            disabled={!isTauriRuntime() || store.loginItemUpdating}
            onCheckedChange={(checked) => void store.setLoginItem(checked)}
            aria-label="Start VibeInspector services at login"
          />
          <span>
            <strong>Start services at login</strong>
            <small>Keep MCP available without opening the desktop window.</small>
          </span>
        </label>
      </section>

      <section class="settings-section shortcuts-setting">
        <div class="section-copy">
          <strong>Keyboard</strong>
          <span>Review navigation and workspace shortcuts.</span>
        </div>
        <Button variant="outline" size="sm" onclick={() => store.openShortcuts()}>
          <Keyboard data-icon="inline-start" />View shortcuts
        </Button>
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>

<style>
  :global(.settings-dialog) {
    border: 1px solid var(--panel-border);
    border-radius: var(--radius-xl);
    box-shadow: var(--dialog-shadow);
  }

  .settings-body {
    display: grid;
  }

  .settings-section,
  .service-section > header,
  .login-setting {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .settings-section,
  .service-section {
    padding: 18px 20px;
  }

  .settings-section + .service-section,
  .service-section + .settings-section {
    border-top: 1px solid var(--border);
  }

  .section-copy {
    display: grid;
    gap: 3px;
  }

  .section-copy strong,
  .service-row strong,
  .login-setting strong {
    font-size: 12.5px;
    font-weight: 610;
  }

  .section-copy span,
  .service-row span,
  .login-setting small,
  .health-meta {
    color: var(--muted-foreground);
    font-size: 11.5px;
    line-height: 1.45;
  }

  .segmented-control {
    display: flex;
    flex: 0 0 auto;
    gap: 3px;
    padding: 3px;
    border: 1px solid color-mix(in oklab, var(--panel-border) 88%, transparent);
    border-radius: var(--radius-lg);
    background: color-mix(in oklab, var(--muted) 84%, transparent);
    box-shadow: inset 0 1px 2px color-mix(in oklab, var(--foreground) 5%, transparent);
  }

  :global(.theme-option) {
    width: 88px;
    border: 1px solid transparent;
    border-radius: calc(var(--radius-lg) - 3px);
    color: var(--muted-foreground);
    background: transparent;
    box-shadow: none;
    transition:
      color var(--dur-micro) var(--ease-out),
      border-color var(--dur-micro) var(--ease-out),
      background-color var(--dur-micro) var(--ease-out);
  }

  :global(.theme-option:hover:not(.selected)) {
    color: var(--foreground);
    background: color-mix(in oklab, var(--background) 34%, transparent);
  }

  :global(.theme-option.selected) {
    border-color: color-mix(in oklab, var(--signal) 32%, var(--panel-border));
    color: var(--foreground);
    background: var(--background);
    box-shadow:
      inset 0 1px 0 color-mix(in oklab, var(--foreground) 6%, transparent),
      0 1px 3px color-mix(in oklab, var(--foreground) 12%, transparent);
  }

  .service-section > header {
    margin-bottom: 14px;
  }

  .health-skeleton,
  .service-rows {
    display: grid;
    gap: 6px;
  }

  .service-row {
    display: grid;
    min-height: 52px;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    background: color-mix(in oklab, var(--muted) 38%, transparent);
  }

  .service-row > div {
    display: grid;
    min-width: 0;
    gap: 2px;
  }

  .service-row > div > span {
    overflow: hidden;
    font-family: var(--app-font-mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .service-icon {
    display: grid;
    color: var(--status-error);
  }

  .service-icon.healthy {
    color: var(--status-success);
  }

  .service-icon :global(svg),
  .settings-error :global(svg) {
    width: 15px;
    height: 15px;
  }

  .health-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin-top: 9px;
    font-family: var(--app-font-mono);
    font-size: 10px;
  }

  .settings-error {
    display: grid;
    grid-template-columns: 16px 1fr;
    gap: 8px;
    margin-top: 10px;
    padding: 9px 10px;
    border: 1px solid color-mix(in oklab, var(--status-error) 28%, var(--border));
    border-radius: var(--radius-md);
    color: var(--status-error);
    background: color-mix(in oklab, var(--status-error) 7%, transparent);
    font-size: 11px;
    line-height: 1.45;
  }

  .login-setting {
    justify-content: start;
    margin-top: 15px;
    padding-top: 15px;
    border-top: 1px solid var(--border);
  }

  .login-setting.disabled {
    opacity: 0.58;
  }

  .login-setting > span {
    display: grid;
    gap: 2px;
  }

  .login-setting small {
    font-weight: 400;
  }

  .shortcuts-setting {
    padding-top: 16px;
    padding-bottom: 16px;
  }
</style>
