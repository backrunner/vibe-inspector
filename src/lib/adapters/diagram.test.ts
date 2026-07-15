import { describe, expect, it } from 'vitest';
import { flowGraphToMermaid } from './diagram';
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
      edges: [{ id: 'edge', from: 'request', to: 'available', label: 'validate' }],
    });

    expect(definition).toContain('flowchart LR');
    expect(definition).toContain('n0(["Checkout request"])');
    expect(definition).toContain('n1{"Items available?"}');
    expect(definition).toContain('n0 -->|"validate"| n1');
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
});
