import { beforeEach, describe, expect, it, vi } from 'vitest';

const projectMocks = vi.hoisted(() => ({
  loadRegisteredProjects: vi.fn(),
  listInspectorProjects: vi.fn(),
}));
const inspectorMocks = vi.hoisted(() => ({
  listCodegraphRelatedFiles: vi.fn(),
  loadInspectorSnapshot: vi.fn(),
  readSourceFile: vi.fn(),
  snapshotModules: vi.fn(),
}));

vi.mock('$app/environment', () => ({ browser: false }));
vi.mock('$app/navigation', () => ({ replaceState: vi.fn() }));

vi.mock('$lib/adapters/tauri', () => ({
  isTauriRuntime: () => true,
}));

vi.mock('$lib/adapters/projects', () => ({
  discoveryErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : 'Project registry unavailable',
  loadRegisteredProjects: projectMocks.loadRegisteredProjects,
  listInspectorProjects: projectMocks.listInspectorProjects,
  registerProjectRoots: vi.fn(),
}));
vi.mock('$lib/adapters/inspector', () => inspectorMocks);

import { seedProjects } from '$domain/entities';
import { InspectorStore } from './inspector.svelte';

describe('InspectorStore workspace restoration', () => {
  beforeEach(() => {
    projectMocks.loadRegisteredProjects.mockReset();
    projectMocks.listInspectorProjects.mockReset();
    inspectorMocks.listCodegraphRelatedFiles.mockReset();
    inspectorMocks.loadInspectorSnapshot.mockReset();
    inspectorMocks.readSourceFile.mockReset();
    inspectorMocks.snapshotModules.mockReset();
    inspectorMocks.listCodegraphRelatedFiles.mockResolvedValue([]);
    inspectorMocks.readSourceFile.mockResolvedValue({
      text: 'export const checkout = true;',
      startLine: 1,
      status: 'current',
      contentHash: 'sha256:current',
    });
  });

  it('keeps restoration failures distinct from first-launch onboarding', async () => {
    const store = new InspectorStore();
    projectMocks.loadRegisteredProjects.mockRejectedValueOnce(
      new Error('Project registry unavailable'),
    );

    await store.restoreRegisteredProjects();

    expect(store.workspaceHydrated).toBe(true);
    expect(store.workspaceError).toBe('Project registry unavailable');
    expect(store.projects).toEqual([]);
  });

  it('clears the restoration error after a successful retry', async () => {
    const store = new InspectorStore();
    projectMocks.loadRegisteredProjects
      .mockRejectedValueOnce(new Error('Project registry unavailable'))
      .mockResolvedValueOnce({ schemaVersion: 1, revision: 0, updatedAt: '0', projects: [] });
    projectMocks.listInspectorProjects.mockResolvedValueOnce([]);

    await store.restoreRegisteredProjects();
    await store.restoreRegisteredProjects();

    expect(store.workspaceHydrated).toBe(true);
    expect(store.workspaceError).toBe('');
    expect(store.needsOnboarding).toBe(true);
  });

  it('starts independent registry and project-list reads together', async () => {
    const store = new InspectorStore();
    let resolveRegistry:
      | ((value: {
          schemaVersion: number;
          revision: number;
          updatedAt: string;
          projects: [];
        }) => void)
      | undefined;
    projectMocks.loadRegisteredProjects.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveRegistry = resolve;
      }),
    );
    projectMocks.listInspectorProjects.mockResolvedValueOnce([]);

    const restoration = store.restoreRegisteredProjects();

    expect(projectMocks.loadRegisteredProjects).toHaveBeenCalledOnce();
    expect(projectMocks.listInspectorProjects).toHaveBeenCalledOnce();

    resolveRegistry?.({ schemaVersion: 1, revision: 0, updatedAt: '0', projects: [] });
    await restoration;

    expect(store.workspaceHydrated).toBe(true);
    expect(store.needsOnboarding).toBe(true);
  });

  it('re-reads the selected snapshot without losing view selection', async () => {
    const store = new InspectorStore();
    const project = structuredClone(seedProjects[1]);
    store.projects = [project];
    store.selectedProjectId = project.id;
    store.selectedModuleId = 'module-checkout';
    store.selectedFlowId = 'flow-checkout';
    store.selectedSourceRefId = 'ref-checkout';
    inspectorMocks.loadInspectorSnapshot.mockResolvedValue({ projectRevision: 8 });
    inspectorMocks.snapshotModules.mockReturnValue(project.modules);

    await store.reloadSelectedProjectSnapshot();

    expect(inspectorMocks.loadInspectorSnapshot).toHaveBeenCalledWith(project.id);
    expect(store.selectedProjectId).toBe(project.id);
    expect(store.selectedModuleId).toBe('module-checkout');
    expect(store.selectedFlowId).toBe('flow-checkout');
    expect(store.selectedSourceRefId).toBe('ref-checkout');
    expect(store.selectedProject?.revision).toBe(8);
  });
});
