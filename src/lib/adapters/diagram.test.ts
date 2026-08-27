import { describe, expect, it } from 'vitest';
import {
  clampDiagramZoom,
  diagramZoomFromWheel,
  flowGraphToMermaid,
  maximumDiagramZoom,
  minimumDiagramZoom,
  relationshipGraph,
} from './diagram';
import type { FlowNode } from '$domain/entities';

function node(id: string, label: string, kind: FlowNode['kind'] = 'action'): FlowNode {
  return { id, label, kind, description: '', sourceRefIds: [], x: 0, y: 0 };
}

describe('flowGraphToMermaid', () => {
  it('serializes renderer-neutral nodes and labeled edges', () => {
    const definition = flowGraphToMermaid({
      nodes: [
        node('request', 'Checkout request', 'start'),
        node('available', 'Items available?', 'decision'),
      ],
      edges: [{ id: 'edge', from: 'request', to: 'available', label: 'validate', kind: 'data', mechanism: 'MCP' }],
    });

    expect(definition).toContain('flowchart LR');
    expect(definition).toContain('n0(["Checkout request"])');
    expect(definition).toContain('n1{"Items available?"}');
    expect(definition).toContain('n0 ==>|"validate · MCP"| n1');
  });

  it('keeps dependency edges visible with a weakened arrow', () => {
    const definition = flowGraphToMermaid({
      nodes: [node('a', 'A'), node('b', 'B')],
      edges: [{ id: 'ab', from: 'a', to: 'b', kind: 'dependency' }],
    });

    expect(definition).toContain('n0 -.-> n1');
    expect(definition).not.toContain('~~~');
  });

  it('uses generated aliases and escapes label syntax', () => {
    const definition = flowGraphToMermaid({
      nodes: [node('unsafe"]\nclick n0 callback', 'Quoted "label" | branch')],
      edges: [],
    });

    expect(definition).toContain('n0["Quoted &quot;label&quot; &#124; branch"]');
    expect(definition).not.toContain('click n0 callback');
  });

  it('supports a top-to-bottom layout for narrow workspaces', () => {
    const definition = flowGraphToMermaid(
      { nodes: [node('start', 'Start', 'start')], edges: [] },
      'TB',
    );

    expect(definition).toContain('flowchart TB');
  });

  it('focuses explicit incoming and outgoing relationships without changing node IDs', () => {
    const graph = {
      nodes: [node('a', 'A'), node('b', 'B'), node('c', 'C')],
      edges: [
        { id: 'ab', from: 'a', to: 'b', kind: 'control' as const },
        { id: 'bc', from: 'b', to: 'c', kind: 'async' as const },
      ],
    };
    expect(relationshipGraph(graph, 'b', 'incoming').edges.map((edge) => edge.id)).toEqual(['ab']);
    expect(relationshipGraph(graph, 'b', 'outgoing').edges.map((edge) => edge.id)).toEqual(['bc']);
    expect(relationshipGraph(graph, 'missing', 'all')).toBe(graph);
  });

  it('maps wheel and pinch deltas to bounded diagram zoom levels', () => {
    expect(diagramZoomFromWheel(1, -100, false)).toBeGreaterThan(1);
    expect(diagramZoomFromWheel(1, 100, false)).toBeLessThan(1);
    expect(diagramZoomFromWheel(1, -1, true)).toBeGreaterThan(1);
    expect(clampDiagramZoom(0.1)).toBe(minimumDiagramZoom);
    expect(clampDiagramZoom(4)).toBe(maximumDiagramZoom);
  });
});
