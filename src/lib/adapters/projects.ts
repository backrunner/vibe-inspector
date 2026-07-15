import { invoke } from '@tauri-apps/api/core';
import { isTauriRuntime } from './tauri';

export type ProjectDiscoveryMode = 'collection' | 'single-project';
export type RegistrationSource = 'manual' | 'codex-history';
export type ProjectKind = 'monorepo' | 'repository' | 'project';
export type ProjectNameSource =
  | 'package-json'
  | 'cargo-toml'
  | 'pyproject-toml'
  | 'deno-json'
  | 'composer-json'
  | 'go-module'
  | 'directory';

export type ProjectCandidate = {
  name: string;
  rootPath: string;
  relativePath: string;
  kind: ProjectKind;
  nameSource: ProjectNameSource;
  markers: string[];
};

export type ProjectDiscoveryResult = {
  selectedRoot: string;
  mode: ProjectDiscoveryMode;
  candidates: ProjectCandidate[];
  visitedDirectories: number;
  truncated: boolean;
};

export type RegisteredProject = {
  id: string;
  name: string;
  rootPath: string;
  kind: ProjectKind;
  nameSource: ProjectNameSource;
  markers: string[];
  source: RegistrationSource;
  sourceEvidence: RegisteredSourceEvidence[];
  revision: number;
  updatedAt: string;
};

export type RegisteredSourceEvidence = {
  id: string;
  kind: RegistrationSource;
  conversationId?: string;
  observedPath: string;
  observedAt: string;
  parserVersion: string;
};

export type RegisterProjectEvidence = {
  rootPath: string;
  source: RegistrationSource;
  conversationIds: string[];
  parserVersion?: string;
};

export type CodexProjectProposal = ProjectCandidate & {
  conversationIds: string[];
  latestActivity: number;
  parserVersion: string;
};

export type CodexHistoryScanResult = {
  provider: string;
  sourcePath: string;
  proposals: CodexProjectProposal[];
  scannedThreads: number;
  ignoredThreads: number;
  truncated: boolean;
};

export type ProjectRegistry = {
  schemaVersion: number;
  revision: number;
  updatedAt: string;
  projects: RegisteredProject[];
};

export type StoredInspectorProject = {
  id: string;
  name: string;
  rootPath: string;
  status: 'ready' | 'stale' | 'error' | 'archived';
  source: string;
  revision: number;
  createdAt: string;
  updatedAt: string;
  archivedAt?: string | null;
};

export async function chooseProjectDirectory(mode: ProjectDiscoveryMode): Promise<string | null> {
  if (!isTauriRuntime()) {
    throw new Error('Folder selection is available in the macOS desktop app.');
  }

  const { open } = await import('@tauri-apps/plugin-dialog');
  const selection = await open({
    directory: true,
    multiple: false,
    title: mode === 'collection' ? 'Choose a projects folder' : 'Choose a project root',
  });

  return typeof selection === 'string' ? selection : null;
}

export async function discoverProjectDirectory(
  rootPath: string,
  mode: ProjectDiscoveryMode,
): Promise<ProjectDiscoveryResult> {
  return invoke<ProjectDiscoveryResult>('discover_projects', {
    input: { rootPath, mode },
  });
}

export async function loadRegisteredProjects(): Promise<ProjectRegistry> {
  return invoke<ProjectRegistry>('load_registered_projects');
}

export async function listInspectorProjects(): Promise<StoredInspectorProject[]> {
  return invoke<StoredInspectorProject[]>('list_inspector_projects');
}

export async function scanCodexHistory(includeArchived = false): Promise<CodexHistoryScanResult> {
  return invoke<CodexHistoryScanResult>('scan_codex_history', {
    input: { includeArchived },
  });
}

export async function registerProjectRoots(
  rootPaths: string[],
  evidence: RegisterProjectEvidence[] = [],
): Promise<ProjectRegistry> {
  return invoke<ProjectRegistry>('register_projects', { input: { rootPaths, evidence } });
}

export function discoveryErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string') return message;
  }
  return 'The selected folder could not be scanned.';
}
