import { invoke } from '@tauri-apps/api/core';
import type { Issue, IssueStatus } from '$domain/entities';
import { isTauriRuntime } from './tauri';

export type ListProjectIssuesOptions = {
  moduleId?: string;
  status?: IssueStatus;
  limit?: number;
};

export async function listProjectIssues(
  projectId: string,
  options: ListProjectIssuesOptions = {},
): Promise<Issue[]> {
  if (!isTauriRuntime()) return [];
  return invoke<Issue[]>('list_project_issues', {
    input: {
      projectId,
      moduleId: options.moduleId ?? null,
      status: options.status ?? null,
      limit: options.limit ?? null,
    },
  });
}

export async function setIssueStatus(
  projectId: string,
  issue: Issue,
  status: IssueStatus,
): Promise<Issue> {
  if (!isTauriRuntime()) {
    throw new Error('Issue tracking is available in the macOS desktop app.');
  }
  return invoke<Issue>('set_issue_status', {
    input: { projectId, issueId: issue.id, expectedRevision: issue.revision, status },
  });
}

export function isIssueClosed(issue: Issue): boolean {
  return issue.status !== 'open';
}

export function formatIssueLocation(issue: Issue): string {
  if (!issue.path) return '';
  if (issue.startLine && issue.endLine && issue.endLine !== issue.startLine) {
    return `${issue.path}:${issue.startLine}-${issue.endLine}`;
  }
  if (issue.startLine) return `${issue.path}:${issue.startLine}`;
  return issue.path;
}
