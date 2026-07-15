import { invoke } from '@tauri-apps/api/core';

export type CommandRun = {
  requestId: string;
  projectId: string;
  command: string;
  scopeFingerprint?: string;
  actor: string;
  status: string;
  resultJson?: string;
  createdAt: string;
  updatedAt: string;
};

export type McpAuditEvent = {
  id: string;
  sessionId: string;
  tool: string;
  projectId?: string;
  entityIds: string[];
  result: string;
  durationMs: number;
  createdAt: string;
};

export type ActivityList = {
  commandRuns: CommandRun[];
  auditEvents: McpAuditEvent[];
};

export async function loadInspectorActivity(projectId: string, limit = 50): Promise<ActivityList> {
  return invoke<ActivityList>('list_inspector_activity', { input: { projectId, limit } });
}

export function formatActivityTime(value: string): string {
  const numeric = Number(value);
  const date = Number.isFinite(numeric) ? new Date(numeric) : new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(date);
}
