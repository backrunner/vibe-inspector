import { describe, expect, it } from 'vitest';
import type { EntityVersion } from '$domain/entities';
import {
  flowVersionSnapshot,
  getEntityVersion,
  listEntityVersions,
  moduleVersionSnapshot,
} from './versions';

function version(overrides: Partial<EntityVersion>): EntityVersion {
  return {
    id: 'version-1',
    projectId: 'project-1',
    entityType: 'flow',
    entityId: 'flow-1',
    revision: 3,
    actor: 'desktop',
    source: 'manual',
    createdAt: '1754000000000',
    snapshotJson: {},
    ...overrides,
  };
}

const flowSnapshot = {
  id: 'flow-1',
  name: 'Checkout',
  summary: 'Reserves inventory.',
  status: 'ready',
  revision: 3,
  updatedAt: '1754000000000',
  graph: {
    nodes: [
      { id: 'start', label: 'Request', kind: 'start', sourceRefIds: ['ref-1'], position: 0 },
      { id: 'pay', label: 'Charge', kind: 'external', description: 'Charges card' },
      { id: 'bogus' },
    ],
    edges: [
      { id: 'e1', from: 'start', to: 'pay', label: 'approved' },
      { from: 'pay', to: 'start' },
      { from: 'pay' },
    ],
  },
  explanation: {
    headline: 'Reserve first',
    body: 'Payment is the boundary.',
    decisions: ['Reserve before charge'],
    confidence: 0.9,
  },
  sourceRefs: [
    { id: 'ref-1', path: 'src/checkout.ts', startLine: 3, endLine: 9, label: 'Orchestration' },
    { path: 'src/other.ts' },
    { nope: true },
  ],
};

describe('flowVersionSnapshot', () => {
  it('maps a stored flow snapshot into renderable data', () => {
    const snapshot = flowVersionSnapshot(version({ snapshotJson: flowSnapshot }));
    expect(snapshot?.name).toBe('Checkout');
    expect(snapshot?.status).toBe('ready');
    expect(snapshot?.graph.nodes).toHaveLength(3);
    expect(snapshot?.graph.nodes[0]).toMatchObject({
      id: 'start',
      kind: 'start',
      description: '',
      sourceRefIds: ['ref-1'],
    });
    expect(snapshot?.graph.nodes[1].kind).toBe('external');
    expect(snapshot?.graph.nodes[2]).toMatchObject({ id: 'bogus', label: 'bogus', kind: 'action' });
    expect(snapshot?.graph.edges).toHaveLength(2);
    expect(snapshot?.graph.edges[1].id).toBe('pay:start');
    expect(snapshot?.explanation?.headline).toBe('Reserve first');
    expect(snapshot?.sourceRefs).toHaveLength(2);
    expect(snapshot?.sourceRefs[1]).toMatchObject({ path: 'src/other.ts', startLine: 1 });
  });

  it('rejects non-flow versions and malformed snapshots', () => {
    expect(flowVersionSnapshot(version({ entityType: 'module' }))).toBeNull();
    expect(flowVersionSnapshot(version({ snapshotJson: 'nope' }))).toBeNull();
    expect(flowVersionSnapshot(version({ snapshotJson: { summary: 'x' } }))).toBeNull();
  });

  it('falls back to safe defaults for unknown enum values', () => {
    const snapshot = flowVersionSnapshot(
      version({
        snapshotJson: {
          name: 'Flow',
          status: 'weird',
          graph: { nodes: [{ id: 'a', label: 'A', kind: 'mystery' }], edges: [] },
        },
      }),
    );
    expect(snapshot?.status).toBe('draft');
    expect(snapshot?.graph.nodes[0].kind).toBe('action');
    expect(snapshot?.explanation).toBeNull();
  });

  it('falls back to workflow/control for unknown diagram and edge kinds', () => {
    const snapshot = flowVersionSnapshot(
      version({
        snapshotJson: {
          name: 'Flow',
          graph: {
            kind: 'mystery',
            nodes: [],
            edges: [{ from: 'a', to: 'b', kind: 'mystery' }],
          },
        },
      }),
    );
    expect(snapshot?.graph.kind).toBe('workflow');
    expect(snapshot?.graph.edges[0].kind).toBe('control');
  });

  it('preserves known diagram and edge kinds', () => {
    const snapshot = flowVersionSnapshot(
      version({
        snapshotJson: {
          name: 'Flow',
          graph: {
            kind: 'sequence',
            nodes: [],
            edges: [{ from: 'a', to: 'b', kind: 'dependency' }],
          },
        },
      }),
    );
    expect(snapshot?.graph.kind).toBe('sequence');
    expect(snapshot?.graph.edges[0].kind).toBe('dependency');
  });

  it('parses edge mechanism and sourceRefIds defensively', () => {
    const snapshot = flowVersionSnapshot(
      version({
        snapshotJson: {
          name: 'Flow',
          graph: {
            nodes: [],
            edges: [
              { from: 'a', to: 'b', mechanism: 'MCP', sourceRefIds: ['ref-1', 7, 'ref-2'] },
              { from: 'b', to: 'c', mechanism: 42 },
            ],
          },
        },
      }),
    );
    expect(snapshot?.graph.edges[0]).toMatchObject({
      mechanism: 'MCP',
      sourceRefIds: ['ref-1', 'ref-2'],
    });
    expect(snapshot?.graph.edges[1]).toMatchObject({ mechanism: undefined, sourceRefIds: [] });
  });
});

describe('moduleVersionSnapshot', () => {
  it('maps a stored module snapshot', () => {
    const snapshot = moduleVersionSnapshot(
      version({
        entityType: 'module',
        snapshotJson: {
          name: 'Payments',
          slug: 'payments',
          summary: 'Provider boundaries.',
          status: 'needs-review',
          parentId: 'module-checkout',
          revision: 7,
          updatedAt: '1754000000000',
        },
      }),
    );
    expect(snapshot).toMatchObject({
      name: 'Payments',
      slug: 'payments',
      status: 'needs-review',
      parentId: 'module-checkout',
      revision: 7,
    });
  });

  it('rejects non-module versions and snapshots without a name', () => {
    expect(moduleVersionSnapshot(version({}))).toBeNull();
    expect(moduleVersionSnapshot(version({ entityType: 'module', snapshotJson: {} }))).toBeNull();
  });
});

describe('browser preview fallbacks', () => {
  it('lists no versions outside the Tauri runtime', async () => {
    await expect(listEntityVersions('project-1', { entityType: 'flow' })).resolves.toEqual([]);
  });

  it('rejects version detail loads outside the Tauri runtime', async () => {
    await expect(getEntityVersion('project-1', 'version-1')).rejects.toThrow('desktop app');
  });
});
