<script lang="ts">
  import InspectorWorkspace from '$components/inspector/InspectorWorkspace.svelte';
  import ModuleRail from '$components/modules/ModuleRail.svelte';
  import ProjectRail from './ProjectRail.svelte';
  import { inspectorStore } from '$stores/inspector.svelte';

  const store = inspectorStore;
</script>

<div
  class:project-collapsed={store.projectRailCollapsed}
  class:module-collapsed={store.moduleRailCollapsed}
  class="registered-workspace"
>
  <ProjectRail />
  <ModuleRail />
  <InspectorWorkspace />
</div>

<style>
  .registered-workspace {
    display: grid;
    min-width: 0;
    min-height: 0;
    flex: 1;
    grid-template-columns: 216px 280px minmax(0, 1fr);
    overflow: hidden;
    transition: grid-template-columns var(--dur-rail) var(--ease-in-out);
  }

  .registered-workspace.project-collapsed {
    grid-template-columns: 56px 280px minmax(0, 1fr);
  }

  .registered-workspace.module-collapsed {
    grid-template-columns: 216px 48px minmax(0, 1fr);
  }

  .registered-workspace.project-collapsed.module-collapsed {
    grid-template-columns: 56px 48px minmax(0, 1fr);
  }

  @media (max-width: 1200px) {
    .registered-workspace {
      grid-template-columns: 196px 252px minmax(0, 1fr);
    }

    .registered-workspace.project-collapsed {
      grid-template-columns: 56px 252px minmax(0, 1fr);
    }

    .registered-workspace.module-collapsed {
      grid-template-columns: 196px 48px minmax(0, 1fr);
    }

    .registered-workspace.project-collapsed.module-collapsed {
      grid-template-columns: 56px 48px minmax(0, 1fr);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .registered-workspace {
      transition: none;
    }
  }
</style>
