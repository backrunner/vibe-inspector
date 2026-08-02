import { describe, expect, it } from 'vitest';
import type { Issue } from '$domain/entities';
import { formatIssueLocation, isIssueClosed, listProjectIssues, setIssueStatus } from './issues';

function issue(overrides: Partial<Issue>): Issue {
  return {
    id: 'issue-1',
    projectId: 'project-1',
    moduleId: 'module-1',
    title: 'Missing retry boundary',
    description: 'The flow charges before confirming inventory.',
    severity: 'p1',
    status: 'open',
    actor: 'mcp',
    source: 'inspector-sync',
    revision: 2,
    createdAt: '1754000000000',
    updatedAt: '1754000000000',
    ...overrides,
  };
}

describe('formatIssueLocation', () => {
  it('formats path and line ranges', () => {
    expect(
      formatIssueLocation(issue({ path: 'src/checkout.ts', startLine: 4, endLine: 9 })),
    ).toBe('src/checkout.ts:4-9');
    expect(formatIssueLocation(issue({ path: 'src/checkout.ts', startLine: 4 }))).toBe(
      'src/checkout.ts:4',
    );
    expect(formatIssueLocation(issue({ path: 'src/checkout.ts' }))).toBe('src/checkout.ts');
    expect(formatIssueLocation(issue({}))).toBe('');
  });

  it('collapses single-line ranges', () => {
    expect(
      formatIssueLocation(issue({ path: 'src/a.ts', startLine: 3, endLine: 3 })),
    ).toBe('src/a.ts:3');
  });
});

describe('isIssueClosed', () => {
  it('treats resolved and wontfix as closed', () => {
    expect(isIssueClosed(issue({}))).toBe(false);
    expect(isIssueClosed(issue({ status: 'resolved' }))).toBe(true);
    expect(isIssueClosed(issue({ status: 'wontfix' }))).toBe(true);
  });
});

describe('browser preview fallbacks', () => {
  it('lists no issues outside the Tauri runtime', async () => {
    await expect(listProjectIssues('project-1', { moduleId: 'module-1' })).resolves.toEqual([]);
  });

  it('rejects status changes outside the Tauri runtime', async () => {
    await expect(setIssueStatus('project-1', issue({}), 'resolved')).rejects.toThrow(
      'desktop app',
    );
  });
});
