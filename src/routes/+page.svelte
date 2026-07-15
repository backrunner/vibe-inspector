<script lang="ts">
  import { onMount, type Component } from 'svelte';
  import WindowTitlebar from '$components/shell/WindowTitlebar.svelte';
  import AppShellSkeleton from '$components/shell/AppShellSkeleton.svelte';
  import ProjectOnboarding from '$components/projects/ProjectOnboarding.svelte';
  import WorkspaceRestoreError from '$components/projects/WorkspaceRestoreError.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
  let RegisteredWorkspace = $state<Component>();
  let ProjectPickerDialog = $state<Component>();
  let ActivityDialog = $state<Component>();
  let SettingsDialog = $state<Component>();
  let ShortcutsDialog = $state<Component>();
  let registeredWorkspaceLoad: Promise<void> | undefined;
  let projectPickerLoad: Promise<void> | undefined;
  let activityLoad: Promise<void> | undefined;
  let settingsLoad: Promise<void> | undefined;
  let shortcutsLoad: Promise<void> | undefined;

  function loadRegisteredWorkspace() {
    return (registeredWorkspaceLoad ??= import('$components/shell/RegisteredWorkspace.svelte').then(
      (module) => {
        RegisteredWorkspace = module.default;
      },
    ));
  }

  function loadProjectPicker() {
    return (projectPickerLoad ??= import('$components/projects/ProjectPickerDialog.svelte').then(
      (module) => {
        ProjectPickerDialog = module.default;
      },
    ));
  }

  function loadActivity() {
    return (activityLoad ??= import('$components/activity/ActivityDialog.svelte').then((module) => {
      ActivityDialog = module.default;
    }));
  }

  function loadSettings() {
    return (settingsLoad ??= import('$components/settings/SettingsDialog.svelte').then((module) => {
      SettingsDialog = module.default;
    }));
  }

  function loadShortcuts() {
    return (shortcutsLoad ??= import('$components/settings/ShortcutsDialog.svelte').then(
      (module) => {
        ShortcutsDialog = module.default;
      },
    ));
  }

  onMount(() => {
    const url = new URL(window.location.href);
    store.hydrateViewState(url);
    if (url.searchParams.has('demo')) store.loadDemoWorkspace();
    else void store.restoreRegisteredProjects();

    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        store.toggleProjectRail();
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        document.querySelector<HTMLInputElement>('.module-search input')?.focus();
      }
      if ((event.metaKey || event.ctrlKey) && event.key === ',') {
        event.preventDefault();
        store.openSettings();
      }
      if ((event.metaKey || event.ctrlKey) && event.key === '/') {
        event.preventDefault();
        store.openShortcuts();
      }
    };
    window.addEventListener('keydown', handleShortcut);
    return () => window.removeEventListener('keydown', handleShortcut);
  });

  $effect(() => {
    if (store.workspaceHydrated && !store.workspaceError && !store.needsOnboarding) {
      void loadRegisteredWorkspace();
    }
    if (store.projectPickerOpen) void loadProjectPicker();
    if (store.activityOpen) void loadActivity();
    if (store.settingsOpen) void loadSettings();
    if (store.shortcutsOpen) void loadShortcuts();
  });
</script>

<svelte:head>
  <title>VibeInspector · Repository observability</title>
  <meta
    name="description"
    content="Inspect repository architecture, business flows, and implementation evidence."
  />
</svelte:head>

<div class="window-frame">
  <WindowTitlebar />
  <div class="app-shell">
    {#if !store.workspaceHydrated}
      <AppShellSkeleton />
    {:else if store.workspaceError}
      <WorkspaceRestoreError />
    {:else if store.needsOnboarding}
      <ProjectOnboarding />
    {:else if RegisteredWorkspace}
      <RegisteredWorkspace />
    {:else}
      <AppShellSkeleton />
    {/if}
  </div>
  {#if ProjectPickerDialog}<ProjectPickerDialog />{/if}
  {#if ActivityDialog}<ActivityDialog />{/if}
  {#if SettingsDialog}<SettingsDialog />{/if}
  {#if ShortcutsDialog}<ShortcutsDialog />{/if}
</div>

<style>
  .window-frame {
    display: flex;
    width: 100vw;
    height: 100dvh;
    flex-direction: column;
    overflow: hidden;
    background: var(--app-backdrop);
  }

  .app-shell {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex: 1;
    overflow: hidden;
    background: transparent;
  }
</style>
