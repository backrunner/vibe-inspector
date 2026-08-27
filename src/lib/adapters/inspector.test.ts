import { describe, expect, it } from 'vitest';
import { snapshotModules, type InspectorSnapshotDto } from './inspector';

describe('snapshotModules', () => {
  it('builds a module tree and preserves inspector provenance', () => {
    const projectId = 'project-1';
    const snapshot: InspectorSnapshotDto = {
      projectId,
      projectRevision: 4,
      modules: [
        {
          id: 'child',
          projectId,
          parentId: 'root',
          name: 'Payments',
          slug: 'payments',
          summary: 'Payment orchestration.',
          status: 'ready',
          revision: 2,
          createdAt: '1',
          updatedAt: '2',
          flows: [
            {
              id: 'flow-1',
              projectId,
              moduleId: 'child',
              name: 'Authorize payment',
              summary: 'Authorizes an idempotent payment intent.',
              status: 'needs-review',
              revision: 3,
              createdAt: '1',
              updatedAt: '3',
              graph: {
                nodes: [
                  {
                    id: 'step-1',
                    label: 'Authorize',
                    kind: 'external',
                    sourceRefIds: ['source-1'],
                    position: 0,
                  },
                ],
                edges: [
                  {
                    id: 'edge-1',
                    from: 'step-1',
                    to: 'step-1',
                    label: null,
                    mechanism: null,
                  },
                ],
              },
              explanation: {
                headline: 'Reuse one payment identity.',
                body: 'Retries resolve the existing provider intent before creating another.',
                decisions: ['Keep the request ID stable.'],
                confidence: 0.91,
                provenance: {
                  source: 'inspector-sync',
                  conversationId: 'conversation-1',
                  model: 'gpt-5',
                  promptHash: 'sha256:prompt',
                  generatedAt: '2026-07-14T02:42:00Z',
                  confidence: 0.91,
                  evidenceIds: ['source-1'],
                },
              },
              sourceRefs: [
                {
                  id: 'source-1',
                  path: 'src/payments.ts',
                  startLine: 8,
                  endLine: 20,
                  contentHash: 'hash',
                  status: 'stale',
                },
              ],
            },
          ],
        },
        {
          id: 'root',
          projectId,
          parentId: null,
          name: 'Commerce',
          slug: 'commerce',
          summary: 'Commerce modules.',
          status: 'ready',
          revision: 1,
          createdAt: '1',
          updatedAt: '1',
          flows: [],
        },
      ],
    };

    const modules = snapshotModules(snapshot);

    expect(modules).toHaveLength(1);
    expect(modules[0].id).toBe('root');
    expect(modules[0].children?.[0].id).toBe('child');
    expect(modules[0].children?.[0].flows[0]).toMatchObject({
      id: 'flow-1',
      revision: 3,
      explanation: {
        confidence: 0.91,
        provenance: {
          source: 'inspector-sync',
          conversationId: 'conversation-1',
          model: 'gpt-5',
          evidenceIds: ['source-1'],
        },
      },
    });
    expect(modules[0].children?.[0].flows[0].sourceRefs[0]).toMatchObject({
      label: 'src/payments.ts',
      status: 'stale',
    });
    expect(modules[0].children?.[0].flows[0].graph.edges[0]).toEqual({
      id: 'edge-1',
      from: 'step-1',
      to: 'step-1',
      label: undefined,
      kind: 'control',
      mechanism: undefined,
      sourceRefIds: [],
    });
  });
});
