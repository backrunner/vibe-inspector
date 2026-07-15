import { beforeEach, describe, expect, it, vi } from 'vitest';

const projectMocks = vi.hoisted(() => ({
  loadRegisteredProjects: vi.fn(),
  listInspectorProjects: vi.fn(),
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

import { InspectorStore } from './inspector.svelte';

describe('InspectorStore workspace restoration', () => {
  beforeEach(() => {
    projectMocks.loadRegisteredProjects.mockReset();
    projectMocks.listInspectorProjects.mockReset();
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
});
