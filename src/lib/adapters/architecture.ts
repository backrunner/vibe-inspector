import type { Module } from '$domain/entities';

export type ArchitectureNode = {
  id: string;
  label: string;
  summary: string;
  depth: number;
  selected: boolean;
};

export type ArchitectureEdge = {
  id: string;
  from: string;
  to: string;
  label: 'contains';
};

export type ArchitectureGraph = {
  nodes: ArchitectureNode[];
  edges: ArchitectureEdge[];
};

function flattenModules(modules: Module[], selectedModuleId: string | undefined, depth = 0): ArchitectureGraph {
  const nodes: ArchitectureNode[] = [];
  const edges: ArchitectureEdge[] = [];
  for (const module of modules) {
    nodes.push({
      id: module.id,
      label: module.name,
      summary: module.summary,
      depth,
      selected: module.id === selectedModuleId,
    });
    for (const child of module.children ?? []) {
      edges.push({ id: `${module.id}:${child.id}`, from: module.id, to: child.id, label: 'contains' });
      const nested = flattenModules([child], selectedModuleId, depth + 1);
      nodes.push(...nested.nodes);
      edges.push(...nested.edges);
    }
  }
  return { nodes, edges };
}

export function moduleArchitectureGraph(
  modules: Module[],
  selectedModuleId?: string,
): ArchitectureGraph {
  return flattenModules(modules, selectedModuleId);
}

function escapeLabel(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll('|', '&#124;')
    .replace(/\s+/g, ' ')
    .trim();
}

export function architectureGraphToMermaid(graph: ArchitectureGraph): string {
  const aliases = new Map(graph.nodes.map((node, index) => [node.id, `m${index}`]));
  const lines = ['flowchart LR'];
  for (const node of graph.nodes) {
    const alias = aliases.get(node.id);
    if (!alias) continue;
    const suffix = node.selected ? ' (selected)' : '';
    lines.push(`  ${alias}["${escapeLabel(node.label + suffix)}"]`);
  }
  for (const edge of graph.edges) {
    const source = aliases.get(edge.from);
    const target = aliases.get(edge.to);
    if (source && target) lines.push(`  ${source} -->|contains| ${target}`);
  }
  return lines.join('\n');
}
