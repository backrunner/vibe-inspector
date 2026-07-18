import { invoke } from '@tauri-apps/api/core';
import type { Flow, Module, Provenance, SourceRef } from '$domain/entities';

export type InspectorSnapshotDto = {
  projectId: string;
  projectRevision: number;
  modules: InspectorModuleDto[];
};

type InspectorModuleDto = {
  id: string;
  projectId: string;
  parentId: string | null;
  name: string;
  slug: string;
  summary: string;
  status: Module['status'];
  revision: number;
  createdAt: string;
  updatedAt: string;
  flows: InspectorFlowDto[];
};

type InspectorFlowDto = {
  id: string;
  projectId: string;
  moduleId: string;
  name: string;
  summary: string;
  status: Flow['status'];
  revision: number;
  createdAt: string;
  updatedAt: string;
  graph: {
    nodes: Array<{
      id: string;
      label: string;
      kind: Flow['graph']['nodes'][number]['kind'];
      description?: string;
      sourceRefIds: string[];
      position: number;
    }>;
    edges: Flow['graph']['edges'];
  };
  explanation: null | {
    headline: string;
    body: string;
    decisions: string[];
    confidence?: number;
    provenance: Provenance;
  };
  sourceRefs: Array<
    Omit<SourceRef, 'symbol' | 'label'> & {
      symbol?: string;
      label?: string;
    }
  >;
};

export type SourceRange = {
  relativePath: string;
  startLine: number;
  endLine: number;
  text: string;
  contentHash: string;
  status: 'current' | 'stale';
};

export async function loadInspectorSnapshot(projectId: string): Promise<InspectorSnapshotDto> {
  return invoke<InspectorSnapshotDto>('load_inspector_snapshot', { input: { projectId } });
}

export async function readSourceFile(projectId: string, source: SourceRef): Promise<SourceRange> {
  return invoke<SourceRange>('read_project_file', {
    input: {
      projectId,
      relativePath: source.path,
      expectedHash: source.contentHash || null,
    },
  });
}

export async function listCodegraphRelatedFiles(
  projectId: string,
  relativePaths: string[],
): Promise<string[]> {
  return invoke<string[]>('list_codegraph_related_files', {
    input: { projectId, relativePaths },
  });
}

export function snapshotModules(snapshot: InspectorSnapshotDto): Module[] {
  const modules = snapshot.modules.map<Module>((module) => ({
    id: module.id,
    name: module.name,
    slug: module.slug,
    summary: module.summary,
    status: module.status,
    revision: module.revision,
    flowCount: module.flows.length,
    children: [],
    flows: module.flows.map<Flow>((flow) => ({
      id: flow.id,
      name: flow.name,
      summary: flow.summary,
      status: flow.status,
      revision: flow.revision,
      owner: flow.explanation?.provenance.source ?? 'Inspector',
      updatedAt: flow.updatedAt,
      graph: {
        nodes: flow.graph.nodes.map((node) => ({
          id: node.id,
          label: node.label,
          kind: node.kind,
          description: node.description ?? '',
          sourceRefIds: node.sourceRefIds,
          x: 0,
          y: 0,
        })),
        edges: flow.graph.edges,
      },
      explanation: flow.explanation
        ? {
            headline: flow.explanation.headline,
            body: flow.explanation.body,
            decisions: flow.explanation.decisions,
            confidence: flow.explanation.confidence ?? 0,
            provenance: flow.explanation.provenance,
          }
        : null,
      sourceRefs: flow.sourceRefs.map((source) => ({
        ...source,
        symbol: source.symbol ?? '',
        label: source.label ?? source.path,
      })),
    })),
  }));
  const byId = new Map(modules.map((module) => [module.id, module]));
  const roots: Module[] = [];

  snapshot.modules.forEach((module, index) => {
    const mapped = modules[index];
    const parent = module.parentId ? byId.get(module.parentId) : undefined;
    if (parent) parent.children?.push(mapped);
    else roots.push(mapped);
  });

  return roots;
}
