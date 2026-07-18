import type { FlowEdge, FlowNode } from '$domain/entities';

type FlowGraph = {
  nodes: FlowNode[];
  edges: FlowEdge[];
};

export const minimumDiagramZoom = 0.7;
export const maximumDiagramZoom = 1.6;

export function clampDiagramZoom(value: number): number {
  return Math.min(maximumDiagramZoom, Math.max(minimumDiagramZoom, value));
}

export function diagramZoomFromWheel(
  currentZoom: number,
  deltaY: number,
  isPinchGesture: boolean,
): number {
  const sensitivity = isPinchGesture ? 0.008 : 0.0015;
  return clampDiagramZoom(currentZoom * Math.exp(-deltaY * sensitivity));
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

function nodeStatement(alias: string, node: FlowNode): string {
  const label = escapeLabel(node.label);

  switch (node.kind) {
    case 'start':
    case 'end':
      return `${alias}(["${label}"])`;
    case 'decision':
      return `${alias}{"${label}"}`;
    case 'external':
      return `${alias}[["${label}"]]`;
    default:
      return `${alias}["${label}"]`;
  }
}

export function flowGraphToMermaid(graph: FlowGraph, direction: 'LR' | 'TB' = 'LR'): string {
  const aliases = new Map(graph.nodes.map((node, index) => [node.id, `n${index}`]));
  const lines = [`flowchart ${direction}`];

  for (const node of graph.nodes) {
    const alias = aliases.get(node.id);
    if (alias) lines.push(`  ${nodeStatement(alias, node)}`);
  }

  for (const edge of graph.edges) {
    const source = aliases.get(edge.from);
    const target = aliases.get(edge.to);
    if (!source || !target) continue;
    const label = edge.label ? `|"${escapeLabel(edge.label)}"|` : '';
    lines.push(`  ${source} -->${label} ${target}`);
  }

  return lines.join('\n');
}
