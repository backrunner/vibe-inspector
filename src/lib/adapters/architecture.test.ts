import { describe, expect, it } from 'vitest';
import type { Module } from '$domain/entities';
import { architectureGraphToMermaid, moduleArchitectureGraph } from './architecture';

function module(id: string, name: string, children: Module[] = []): Module {
  return {
    id,
    name,
    slug: id,
    summary: `${name} responsibility`,
    status: 'ready',
    flowCount: 0,
    children,
    flows: [],
  };
}

describe('moduleArchitectureGraph', () => {
  it('represents module containment separately from business-flow edges', () => {
    const graph = moduleArchitectureGraph([
      module('platform', 'Platform', [module('storage', 'Storage')]),
      module('desktop', 'Desktop'),
    ], 'storage');

    expect(graph.nodes.map((node) => node.id)).toEqual(['platform', 'storage', 'desktop']);
    expect(graph.edges).toEqual([
      { id: 'platform:storage', from: 'platform', to: 'storage', label: 'contains' },
    ]);
    expect(graph.nodes.find((node) => node.id === 'storage')?.selected).toBe(true);
  });

  it('renders stable aliases and escapes architecture labels', () => {
    const definition = architectureGraphToMermaid(
      moduleArchitectureGraph([module('a', 'Core <Runtime>')]),
    );
    expect(definition).toContain('flowchart LR');
    expect(definition).toContain('m0["Core &lt;Runtime&gt;"]');
  });

  it('returns an empty graph for empty modules', () => {
    const graph = moduleArchitectureGraph([]);
    expect(graph).toEqual({ nodes: [], edges: [] });
    expect(architectureGraphToMermaid(graph)).toBe('flowchart LR');
  });

  it('tracks depth across deeply nested modules', () => {
    const graph = moduleArchitectureGraph([
      module('platform', 'Platform', [
        module('storage', 'Storage', [module('sqlite', 'SQLite')]),
      ]),
    ]);

    expect(graph.nodes.map((node) => [node.id, node.depth])).toEqual([
      ['platform', 0],
      ['storage', 1],
      ['sqlite', 2],
    ]);
    expect(graph.edges.map((edge) => edge.id)).toEqual(['platform:storage', 'storage:sqlite']);
  });

  it('marks nothing as selected when selectedModuleId does not exist', () => {
    const graph = moduleArchitectureGraph(
      [module('platform', 'Platform', [module('storage', 'Storage')])],
      'missing',
    );

    expect(graph.nodes.every((node) => !node.selected)).toBe(true);
  });
});
