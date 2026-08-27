import type { FlowEdge, FlowEdgeKind, FlowNode } from '$domain/entities';

type FlowGraph = {
  nodes: FlowNode[];
  edges: FlowEdge[];
};

export type RelationshipDirection = 'all' | 'incoming' | 'outgoing';

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

export function relationshipGraph(
  graph: FlowGraph,
  nodeId: string | undefined,
  direction: RelationshipDirection = 'all',
): FlowGraph {
  if (!nodeId || !graph.nodes.some((node) => node.id === nodeId)) return graph;
  const edges = graph.edges.filter((edge) => {
    if (direction === 'incoming') return edge.to === nodeId;
    if (direction === 'outgoing') return edge.from === nodeId;
    return edge.from === nodeId || edge.to === nodeId;
  });
  const nodeIds = new Set([nodeId]);
  for (const edge of edges) {
    nodeIds.add(edge.from);
    nodeIds.add(edge.to);
  }
  return {
    ...graph,
    nodes: graph.nodes.filter((node) => nodeIds.has(node.id)),
    edges,
  };
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

function edgeArrow(kind: FlowEdgeKind | undefined): string {
  switch (kind) {
    case 'data':
      return '==>';
    case 'async':
      return '-.->';
    case 'error':
      return '--x';
    case 'dependency':
      return '-.->';
    default:
      return '-->';
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
    const labelText = [edge.label, edge.mechanism].filter(Boolean).join(' · ');
    const label = labelText ? `|"${escapeLabel(labelText)}"|` : '';
    lines.push(`  ${source} ${edgeArrow(edge.kind)}${label} ${target}`);
  }

  return lines.join('\n');
}
