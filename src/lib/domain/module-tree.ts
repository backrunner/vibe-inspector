import type { Flow, Module } from './entities';

export type ModuleTreeRow =
  | { kind: 'module'; module: Module; depth: number; parentId?: string }
  | { kind: 'flow'; module: Module; flow: Flow; depth: number; parentId: string };

export type ModuleTreeModel = {
  rows: ModuleTreeRow[];
  flowCounts: Map<string, number>;
};

export function buildModuleTree(
  modules: Module[],
  expanded: ReadonlySet<string>,
  rawQuery: string,
): ModuleTreeModel {
  const query = rawQuery.trim().toLowerCase();
  const matchingModuleIds = new Set<string>();
  if (query) collectMatchingModuleIds(modules, query, matchingModuleIds);

  const flowCounts = new Map<string, number>();
  countFlows(modules, flowCounts);

  return {
    rows: collectRows(modules, expanded, query, matchingModuleIds, new Set()),
    flowCounts,
  };
}

export function moduleAncestorIds(modules: Module[], moduleId: string): string[] {
  for (const module of modules) {
    if (module.id === moduleId) return [module.id];
    const childPath = moduleAncestorIds(module.children ?? [], moduleId);
    if (childPath.length > 0) return [module.id, ...childPath];
  }
  return [];
}

function collectRows(
  modules: Module[],
  expanded: ReadonlySet<string>,
  query: string,
  matchingModuleIds: ReadonlySet<string>,
  seenFlows: Set<string>,
  depth = 0,
  parentId?: string,
): ModuleTreeRow[] {
  const rows: ModuleTreeRow[] = [];

  for (const module of modules) {
    if (query && !matchingModuleIds.has(module.id)) continue;
    rows.push({ kind: 'module', module, depth, parentId });

    if (!query && !expanded.has(module.id)) continue;
    const moduleIsMatch =
      Boolean(query) && (textMatches(module.name, query) || textMatches(module.summary, query));

    for (const flow of module.flows) {
      if (seenFlows.has(flow.id)) continue;
      if (
        query &&
        !moduleIsMatch &&
        !textMatches(flow.name, query) &&
        !textMatches(flow.summary, query)
      ) {
        continue;
      }
      seenFlows.add(flow.id);
      rows.push({ kind: 'flow', module, flow, depth: depth + 1, parentId: module.id });
    }

    rows.push(
      ...collectRows(
        module.children ?? [],
        expanded,
        query,
        matchingModuleIds,
        seenFlows,
        depth + 1,
        module.id,
      ),
    );
  }

  return rows;
}

function collectMatchingModuleIds(
  modules: Module[],
  query: string,
  matches: Set<string>,
): boolean {
  let branchMatches = false;
  for (const module of modules) {
    const childMatches = collectMatchingModuleIds(module.children ?? [], query, matches);
    const ownMatch =
      textMatches(module.name, query) ||
      textMatches(module.summary, query) ||
      module.flows.some(
        (flow) => textMatches(flow.name, query) || textMatches(flow.summary, query),
      );
    if (ownMatch || childMatches) {
      matches.add(module.id);
      branchMatches = true;
    }
  }
  return branchMatches;
}

function countFlows(modules: Module[], counts: Map<string, number>): number {
  let total = 0;
  for (const module of modules) {
    const count = module.flows.length + countFlows(module.children ?? [], counts);
    counts.set(module.id, count);
    total += count;
  }
  return total;
}

function textMatches(value: string, query: string): boolean {
  return value.toLowerCase().includes(query);
}
