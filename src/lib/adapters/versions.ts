import { invoke } from '@tauri-apps/api/core';
import type {
  EntityStatus,
  EntityVersion,
  EntityVersionSummary,
  DiagramKind,
  FlowEdge,
  FlowEdgeKind,
  FlowNodeKind,
} from '$domain/entities';
import { isTauriRuntime } from './tauri';

export type ListEntityVersionsOptions = {
  entityType?: 'module' | 'flow';
  entityId?: string;
  limit?: number;
};

export async function listEntityVersions(
  projectId: string,
  options: ListEntityVersionsOptions = {},
): Promise<EntityVersionSummary[]> {
  if (!isTauriRuntime()) return [];
  return invoke<EntityVersionSummary[]>('list_entity_versions', {
    input: {
      projectId,
      entityType: options.entityType ?? null,
      entityId: options.entityId ?? null,
      limit: options.limit ?? null,
    },
  });
}

export async function getEntityVersion(
  projectId: string,
  versionId: string,
): Promise<EntityVersion> {
  if (!isTauriRuntime()) {
    throw new Error('Version history is available in the macOS desktop app.');
  }
  return invoke<EntityVersion>('get_entity_version', { input: { projectId, versionId } });
}

export type VersionedFlowSnapshot = {
  name: string;
  summary: string;
  status: EntityStatus;
  revision?: number;
  updatedAt?: string;
  graph: {
    kind: DiagramKind;
    nodes: Array<{
      id: string;
      label: string;
      kind: FlowNodeKind;
      description: string;
      sourceRefIds: string[];
    }>;
    edges: FlowEdge[];
  };
  explanation: {
    headline: string;
    body: string;
    decisions: string[];
    confidence: number;
  } | null;
  sourceRefs: Array<{
    id: string;
    path: string;
    startLine: number;
    endLine: number;
    symbol: string;
    label: string;
  }>;
};

export type VersionedModuleSnapshot = {
  name: string;
  slug: string;
  summary: string;
  status: EntityStatus;
  parentId?: string | null;
  revision?: number;
  updatedAt?: string;
};

function asRecord(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

function asString(value: unknown, fallback = ''): string {
  return typeof value === 'string' ? value : fallback;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function asStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
}

const flowNodeKinds: FlowNodeKind[] = ['start', 'action', 'decision', 'external', 'end'];
const diagramKinds: DiagramKind[] = ['architecture', 'workflow', 'sequence', 'dataflow', 'lifecycle'];
const flowEdgeKinds: FlowEdgeKind[] = ['control', 'data', 'async', 'error', 'dependency'];
const entityStatuses: EntityStatus[] = ['draft', 'ready', 'stale', 'needs-review', 'archived'];

function asFlowNodeKind(value: unknown): FlowNodeKind {
  return flowNodeKinds.includes(value as FlowNodeKind) ? (value as FlowNodeKind) : 'action';
}

function asEntityStatus(value: unknown): EntityStatus {
  return entityStatuses.includes(value as EntityStatus) ? (value as EntityStatus) : 'draft';
}

function asDiagramKind(value: unknown): DiagramKind {
  return diagramKinds.includes(value as DiagramKind) ? (value as DiagramKind) : 'workflow';
}

function asFlowEdgeKind(value: unknown): FlowEdgeKind {
  return flowEdgeKinds.includes(value as FlowEdgeKind) ? (value as FlowEdgeKind) : 'control';
}

export function flowVersionSnapshot(version: EntityVersion): VersionedFlowSnapshot | null {
  if (version.entityType !== 'flow') return null;
  const snapshot = asRecord(version.snapshotJson);
  if (!snapshot || typeof snapshot.name !== 'string') return null;
  const graph = asRecord(snapshot.graph);
  const nodes = Array.isArray(graph?.nodes) ? graph.nodes : [];
  const edges = Array.isArray(graph?.edges) ? graph.edges : [];
  const explanation = asRecord(snapshot.explanation);
  const sourceRefs = Array.isArray(snapshot.sourceRefs) ? snapshot.sourceRefs : [];
  return {
    name: snapshot.name,
    summary: asString(snapshot.summary),
    status: asEntityStatus(snapshot.status),
    revision: asNumber(snapshot.revision),
    updatedAt: typeof snapshot.updatedAt === 'string' ? snapshot.updatedAt : undefined,
    graph: {
      kind: asDiagramKind(graph?.kind),
      nodes: nodes.flatMap((node) => {
        const record = asRecord(node);
        if (!record || typeof record.id !== 'string') return [];
        return [
          {
            id: record.id,
            label: asString(record.label, record.id),
            kind: asFlowNodeKind(record.kind),
            description: asString(record.description),
            sourceRefIds: asStringArray(record.sourceRefIds),
          },
        ];
      }),
      edges: edges.flatMap((edge) => {
        const record = asRecord(edge);
        if (!record || typeof record.from !== 'string' || typeof record.to !== 'string') return [];
        return [
          {
            id: asString(record.id, `${record.from}:${record.to}`),
            from: record.from,
            to: record.to,
            label: typeof record.label === 'string' ? record.label : undefined,
            kind: asFlowEdgeKind(record.kind),
            mechanism: typeof record.mechanism === 'string' ? record.mechanism : undefined,
            sourceRefIds: asStringArray(record.sourceRefIds),
          },
        ];
      }),
    },
    explanation: explanation
      ? {
          headline: asString(explanation.headline),
          body: asString(explanation.body),
          decisions: asStringArray(explanation.decisions),
          confidence: asNumber(explanation.confidence) ?? 0,
        }
      : null,
    sourceRefs: sourceRefs.flatMap((sourceRef) => {
      const record = asRecord(sourceRef);
      if (!record || typeof record.path !== 'string') return [];
      return [
        {
          id: asString(record.id, record.path),
          path: record.path,
          startLine: asNumber(record.startLine) ?? 1,
          endLine: asNumber(record.endLine) ?? asNumber(record.startLine) ?? 1,
          symbol: asString(record.symbol),
          label: asString(record.label, record.path),
        },
      ];
    }),
  };
}

export function moduleVersionSnapshot(version: EntityVersion): VersionedModuleSnapshot | null {
  if (version.entityType !== 'module') return null;
  const snapshot = asRecord(version.snapshotJson);
  if (!snapshot || typeof snapshot.name !== 'string') return null;
  return {
    name: snapshot.name,
    slug: asString(snapshot.slug),
    summary: asString(snapshot.summary),
    status: asEntityStatus(snapshot.status),
    parentId: typeof snapshot.parentId === 'string' ? snapshot.parentId : null,
    revision: asNumber(snapshot.revision),
    updatedAt: typeof snapshot.updatedAt === 'string' ? snapshot.updatedAt : undefined,
  };
}
