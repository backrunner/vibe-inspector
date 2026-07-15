import { describe, expect, it } from 'vitest';
import type { Flow, Module } from './entities';
import { buildModuleTree, moduleAncestorIds } from './module-tree';

function flow(id: string): Flow {
  return {
    id,
    name: `Flow ${id}`,
    summary: `Summary ${id}`,
    status: 'ready',
    owner: 'Test',
    updatedAt: 'now',
    graph: { nodes: [], edges: [] },
    explanation: null,
    sourceRefs: [],
  };
}

function module(id: string, children: Module[] = []): Module {
  return {
    id,
    name: `Module ${id}`,
    slug: id,
    summary: `Summary ${id}`,
    status: 'ready',
    flowCount: 1,
    flows: [flow(`flow-${id}`)],
    children,
  };
}

describe('buildModuleTree', () => {
  it('counts descendant flows once and preserves tree ancestry', () => {
    const modules = [module('root', [module('child', [module('leaf')])])];
    const model = buildModuleTree(modules, new Set(['root', 'child', 'leaf']), '');

    expect(model.flowCounts.get('root')).toBe(3);
    expect(model.flowCounts.get('child')).toBe(2);
    expect(moduleAncestorIds(modules, 'leaf')).toEqual(['root', 'child', 'leaf']);
    expect(model.rows.map((row) => row.kind)).toEqual([
      'module',
      'flow',
      'module',
      'flow',
      'module',
      'flow',
    ]);
  });

  it('builds a 1,000-module visible model without recursive row recounting', () => {
    const modules = Array.from({ length: 1_000 }, (_, index) => module(String(index)));
    const expanded = new Set(modules.map((item) => item.id));
    const model = buildModuleTree(modules, expanded, '');

    expect(model.rows).toHaveLength(2_000);
    expect(model.flowCounts.size).toBe(1_000);
    expect(model.flowCounts.get('999')).toBe(1);
  });

  it('keeps ancestors visible when a nested flow matches a filter', () => {
    const modules = [module('root', [module('payments')])];
    modules[0].children![0].flows[0].name = 'Authorize card';
    const model = buildModuleTree(modules, new Set(), 'authorize');

    expect(model.rows.map((row) => (row.kind === 'module' ? row.module.id : row.flow.id))).toEqual([
      'root',
      'payments',
      'flow-payments',
    ]);
  });
});
