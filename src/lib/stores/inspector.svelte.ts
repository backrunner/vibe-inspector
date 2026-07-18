import { browser } from '$app/environment';
import { replaceState as replaceRouteState } from '$app/navigation';
import {
  findFirstFlow,
  findModule,
  flattenModules,
  seedProjects,
  type Flow,
  type Module,
  type Project,
  type SourceRef,
} from '$domain/entities';
import {
  discoveryErrorMessage,
  listInspectorProjects,
  loadRegisteredProjects,
  registerProjectRoots,
  type ProjectCandidate,
  type RegisterProjectEvidence,
  type RegisteredProject,
  type StoredInspectorProject,
} from '$lib/adapters/projects';
import {
  listCodegraphRelatedFiles,
  loadInspectorSnapshot,
  readSourceFile,
  snapshotModules,
} from '$lib/adapters/inspector';
import { loadInspectorActivity, type CommandRun, type McpAuditEvent } from '$lib/adapters/activity';
import { loadRuntimeHealth, updateLoginItem, type RuntimeHealth } from '$lib/adapters/runtime';
import { isTauriRuntime } from '$lib/adapters/tauri';

export class InspectorStore {
  projects = $state<Project[]>([]);
  selectedProjectId = $state('');
  selectedModuleId = $state('');
  selectedFlowId = $state('');
  selectedSourceRefId = $state('');
  projectRailCollapsed = $state(false);
  moduleRailCollapsed = $state(false);
  projectPickerOpen = $state(false);
  activityOpen = $state(false);
  settingsOpen = $state(false);
  shortcutsOpen = $state(false);
  workspaceHydrated = $state(false);
  workspaceError = $state('');
  theme = $state<'dark' | 'light'>('dark');
  searchQuery = $state('');
  inspectorStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  inspectorError = $state('');
  sourceStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  sourceError = $state('');
  sourceCode = $state('');
  sourceStartLine = $state(1);
  sourceReadStatus = $state<'current' | 'stale' | 'unreadable'>('current');
  relatedSourceRefs = $state<SourceRef[]>([]);
  activityStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  activityError = $state('');
  commandRuns = $state<CommandRun[]>([]);
  auditEvents = $state<McpAuditEvent[]>([]);
  runtimeHealthStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  runtimeHealthError = $state('');
  runtimeHealth = $state<RuntimeHealth>();
  loginItemUpdating = $state(false);
  private inspectorRequest = 0;
  private sourceRequest = 0;
  private relatedSourceRequest = 0;
  private viewStateFrame = 0;
  private pendingSelection: {
    projectId?: string;
    moduleId?: string;
    flowId?: string;
    sourceRefId?: string;
  } = {};

  get selectedProject(): Project | undefined {
    return this.projects.find((project) => project.id === this.selectedProjectId);
  }

  get needsOnboarding(): boolean {
    return this.projects.length === 0;
  }

  get selectedModule(): Module | undefined {
    return findModule(this.selectedProject, this.selectedModuleId);
  }

  get selectedFlow(): Flow | undefined {
    return (
      this.selectedModule?.flows.find((flow) => flow.id === this.selectedFlowId) ??
      this.selectedModule?.flows[0]
    );
  }

  get selectedSourceRef(): SourceRef | undefined {
    return (
      this.visibleSourceRefs.find((ref) => ref.id === this.selectedSourceRefId) ??
      this.visibleSourceRefs[0]
    );
  }

  get visibleSourceRefs(): SourceRef[] {
    return this.selectedFlow ? [...this.selectedFlow.sourceRefs, ...this.relatedSourceRefs] : [];
  }

  get visibleModules(): Module[] {
    return this.selectedProject ? flattenModules(this.selectedProject.modules) : [];
  }

  get totalFlowCount(): number {
    return this.visibleModules.reduce((total, module) => total + module.flows.length, 0);
  }

  selectProject(projectId: string) {
    const project = this.projects.find((item) => item.id === projectId);
    if (!project) return;
    this.selectedProjectId = project.id;
    this.selectedModuleId = '';
    this.selectedFlowId = '';
    this.selectedSourceRefId = '';
    this.relatedSourceRefs = [];
    this.sourceCode = '';
    this.sourceStatus = 'idle';
    this.persistViewState();
    if (isTauriRuntime()) void this.hydrateProjectInspector(project.id, false);
    else this.selectFirstProjectFlow(project);
  }

  hydrateViewState(url: URL) {
    if (!browser) return;
    this.pendingSelection = {
      projectId: url.searchParams.get('project') ?? undefined,
      moduleId: url.searchParams.get('module') ?? undefined,
      flowId: url.searchParams.get('flow') ?? undefined,
      sourceRefId: url.searchParams.get('file') ?? undefined,
    };
    const storedTheme = localStorage.getItem('vibe-inspector.theme');
    this.theme =
      storedTheme === 'light' || storedTheme === 'dark'
        ? storedTheme
        : window.matchMedia('(prefers-color-scheme: dark)').matches
          ? 'dark'
          : 'light';
    this.projectRailCollapsed = localStorage.getItem('vibe-inspector.project-rail') === 'collapsed';
    this.moduleRailCollapsed = localStorage.getItem('vibe-inspector.module-rail') === 'collapsed';
    document.documentElement.classList.toggle('dark', this.theme === 'dark');
    document.documentElement.style.colorScheme = this.theme;
  }

  openProjectPicker() {
    this.projectPickerOpen = true;
  }

  closeProjectPicker() {
    this.projectPickerOpen = false;
  }

  async openActivity() {
    this.activityOpen = true;
    this.activityError = '';
    if (!this.selectedProjectId || !isTauriRuntime()) {
      this.activityStatus = 'ready';
      this.commandRuns = [];
      this.auditEvents = [];
      return;
    }
    this.activityStatus = 'loading';
    try {
      const activity = await loadInspectorActivity(this.selectedProjectId);
      this.commandRuns = activity.commandRuns;
      this.auditEvents = activity.auditEvents;
      this.activityStatus = 'ready';
    } catch (error) {
      this.activityStatus = 'error';
      this.activityError = discoveryErrorMessage(error);
    }
  }

  openSettings() {
    this.settingsOpen = true;
    void this.loadRuntimeHealthSnapshot();
  }

  openShortcuts() {
    this.settingsOpen = false;
    this.shortcutsOpen = true;
  }

  async loadRuntimeHealthSnapshot() {
    this.runtimeHealthError = '';
    if (!isTauriRuntime()) {
      this.runtimeHealth = {
        app: 'browser-preview',
        tray: 'unavailable',
        mcp: 'unavailable',
        loginItemEnabled: false,
        checkedAt: String(Date.now()),
      };
      this.runtimeHealthStatus = 'ready';
      return;
    }

    this.runtimeHealthStatus = 'loading';
    try {
      this.runtimeHealth = await loadRuntimeHealth();
      this.runtimeHealthStatus = 'ready';
    } catch (error) {
      this.runtimeHealthStatus = 'error';
      this.runtimeHealthError = discoveryErrorMessage(error);
    }
  }

  async setLoginItem(enabled: boolean) {
    if (!isTauriRuntime() || this.loginItemUpdating) return;
    this.loginItemUpdating = true;
    this.runtimeHealthError = '';
    try {
      this.runtimeHealth = await updateLoginItem(enabled);
      this.runtimeHealthStatus = 'ready';
    } catch (error) {
      this.runtimeHealthStatus = 'error';
      this.runtimeHealthError = discoveryErrorMessage(error);
    } finally {
      this.loginItemUpdating = false;
    }
  }

  async restoreRegisteredProjects() {
    this.workspaceHydrated = false;
    this.workspaceError = '';
    if (!isTauriRuntime()) {
      this.workspaceHydrated = true;
      return;
    }

    try {
      const [registry, storedProjects] = await Promise.all([
        loadRegisteredProjects(),
        listInspectorProjects(),
      ]);
      this.applyRegisteredProjects(registry.projects, storedProjects);
      const project =
        this.projects.find((item) => item.id === this.pendingSelection.projectId) ??
        this.projects.find((item) => item.id === this.selectedProjectId) ??
        this.projects[0];
      if (project) {
        this.selectedProjectId = project.id;
        await this.hydrateProjectInspector(project.id, true);
      }
    } catch (error) {
      this.workspaceError = discoveryErrorMessage(error);
      this.projects = [];
    } finally {
      this.workspaceHydrated = true;
    }
  }

  async registerProjectCandidates(
    candidates: ProjectCandidate[],
    evidence: RegisterProjectEvidence[] = [],
  ) {
    if (isTauriRuntime()) {
      const registry = await registerProjectRoots(
        candidates.map((candidate) => candidate.rootPath),
        evidence,
      );
      const storedProjects = await listInspectorProjects();
      const previousRoots = new Set(this.projects.map((project) => project.rootPath));
      this.applyRegisteredProjects(registry.projects, storedProjects);
      const firstAdded = this.projects.find((project) => !previousRoots.has(project.rootPath));
      if (firstAdded) this.selectProject(firstAdded.id);
      this.projectPickerOpen = false;
      return;
    }

    const existingRoots = new Set(this.projects.map((project) => project.rootPath));
    const added = candidates
      .filter((candidate) => !existingRoots.has(candidate.rootPath))
      .map<Project>((candidate) => ({
        id: crypto.randomUUID(),
        name: candidate.name,
        rootPath: candidate.rootPath,
        status: 'ready',
        branch: candidate.kind === 'monorepo' ? 'workspace' : 'repository',
        lastScanAt: 'Not inspected',
        modules: [],
        source: evidence.find((item) => item.rootPath === candidate.rootPath)?.source ?? 'manual',
        conversationCount:
          evidence.find((item) => item.rootPath === candidate.rootPath)?.conversationIds.length ??
          0,
      }));

    if (added.length === 0) {
      this.projectPickerOpen = false;
      return;
    }

    this.projects = [...this.projects, ...added];
    this.selectProject(added[0].id);
    this.projectPickerOpen = false;
  }

  loadDemoWorkspace() {
    this.projects = seedProjects;
    this.selectedProjectId = 'project-vibe-inspector';
    this.selectedModuleId = 'module-inspector-ui';
    this.selectedFlowId = 'flow-inspector-refresh';
    this.selectedSourceRefId = 'ref-inspector';
    this.applyPendingSelection();
    this.workspaceHydrated = true;
  }

  private applyRegisteredProjects(
    registrations: RegisteredProject[],
    storedProjects: StoredInspectorProject[] = [],
  ) {
    const previousByRoot = new Map(this.projects.map((project) => [project.rootPath, project]));
    const storedById = new Map(storedProjects.map((project) => [project.id, project]));
    this.projects = registrations.map<Project>((registration) => {
      const previous = previousByRoot.get(registration.rootPath);
      const stored = storedById.get(registration.id);
      return {
        id: registration.id,
        name: registration.name,
        rootPath: registration.rootPath,
        status: stored?.status === 'archived' ? 'stale' : (stored?.status ?? 'ready'),
        branch: registration.kind === 'monorepo' ? 'workspace' : 'repository',
        lastScanAt: formatRelativeActivity(stored?.updatedAt ?? registration.updatedAt),
        modules: previous?.modules ?? [],
        source: registration.source,
        conversationCount:
          registration.sourceEvidence.filter((evidence) => evidence.conversationId).length ||
          previous?.conversationCount ||
          0,
        revision: stored?.revision ?? registration.revision,
      };
    });
  }

  selectModule(moduleId: string) {
    const module = findModule(this.selectedProject, moduleId);
    if (!module) return;
    this.selectedModuleId = module.id;
    const flow = module.flows[0];
    this.selectedFlowId = flow?.id ?? '';
    this.selectedSourceRefId = flow?.sourceRefs[0]?.id ?? '';
    this.relatedSourceRefs = [];
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
  }

  selectFlow(moduleId: string, flowId: string) {
    const module = findModule(this.selectedProject, moduleId);
    const flow = module?.flows.find((item) => item.id === flowId);
    if (!module || !flow) return;
    this.selectedModuleId = module.id;
    this.selectedFlowId = flow.id;
    this.selectedSourceRefId = flow.sourceRefs[0]?.id ?? '';
    this.relatedSourceRefs = [];
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
  }

  selectSourceRef(refId: string) {
    this.selectedSourceRefId = refId;
    this.persistViewState();
    void this.loadSelectedSource();
  }

  toggleProjectRail() {
    this.projectRailCollapsed = !this.projectRailCollapsed;
    if (browser) {
      localStorage.setItem(
        'vibe-inspector.project-rail',
        this.projectRailCollapsed ? 'collapsed' : 'expanded',
      );
    }
  }

  toggleModuleRail() {
    this.moduleRailCollapsed = !this.moduleRailCollapsed;
    if (browser) {
      localStorage.setItem(
        'vibe-inspector.module-rail',
        this.moduleRailCollapsed ? 'collapsed' : 'expanded',
      );
    }
  }

  toggleTheme() {
    this.setTheme(this.theme === 'dark' ? 'light' : 'dark');
  }

  setTheme(theme: 'dark' | 'light') {
    this.theme = theme;
    if (browser) {
      document.documentElement.classList.toggle('dark', this.theme === 'dark');
      document.documentElement.style.colorScheme = this.theme;
      localStorage.setItem('vibe-inspector.theme', this.theme);
    }
  }

  async reloadSelectedProjectSnapshot() {
    if (!this.selectedProjectId || !isTauriRuntime()) return;
    this.pendingSelection = {
      projectId: this.selectedProjectId,
      moduleId: this.selectedModuleId,
      flowId: this.selectedFlowId,
      sourceRefId: this.selectedSourceRefId,
    };
    await this.hydrateProjectInspector(this.selectedProjectId, true);
  }

  private async hydrateProjectInspector(projectId: string, restoreSelection: boolean) {
    const request = ++this.inspectorRequest;
    this.inspectorStatus = 'loading';
    this.inspectorError = '';
    try {
      const snapshot = await loadInspectorSnapshot(projectId);
      if (request !== this.inspectorRequest) return;
      const modules = snapshotModules(snapshot);
      this.projects = this.projects.map((project) =>
        project.id === projectId
          ? { ...project, modules, revision: snapshot.projectRevision }
          : project,
      );
      this.inspectorStatus = 'ready';
      if (this.selectedProjectId !== projectId) return;
      const project = this.projects.find((item) => item.id === projectId);
      if (!project) return;
      if (restoreSelection) this.applyPendingSelection();
      else this.selectFirstProjectFlow(project);
    } catch (error) {
      if (request !== this.inspectorRequest) return;
      this.inspectorStatus = 'error';
      this.inspectorError = discoveryErrorMessage(error);
    }
  }

  private selectFirstProjectFlow(project: Project) {
    const first = findFirstFlow(project);
    this.selectedModuleId = first?.module.id ?? project.modules[0]?.id ?? '';
    this.selectedFlowId = first?.flow.id ?? '';
    this.selectedSourceRefId = first?.flow.sourceRefs[0]?.id ?? '';
    this.relatedSourceRefs = [];
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
  }

  private async loadSelectedSource() {
    const project = this.selectedProject;
    const flow = this.selectedFlow;
    const source = this.selectedSourceRef;
    const request = ++this.sourceRequest;
    this.sourceError = '';
    this.sourceCode = '';
    this.sourceStartLine = 1;
    this.sourceReadStatus = source?.status ?? 'current';

    if (!project || !flow || !source) {
      this.sourceStatus = 'idle';
      return;
    }

    if (!isTauriRuntime()) {
      const code = flow.files?.[source.path];
      if (code === undefined) {
        this.sourceStatus = 'error';
        this.sourceError = 'Source is unavailable in this preview.';
        this.sourceReadStatus = 'unreadable';
        return;
      }
      this.sourceCode =
        source.origin === 'codegraph'
          ? code
          : previewSourceFile(code, source.startLine, source.endLine);
      this.sourceStartLine = 1;
      this.sourceStatus = 'ready';
      return;
    }

    this.sourceStatus = 'loading';
    try {
      const result = await readSourceFile(project.id, source);
      if (request !== this.sourceRequest) return;
      this.sourceCode = result.text;
      this.sourceStartLine = result.startLine;
      this.sourceReadStatus = result.status;
      source.status = result.status;
      source.contentHash = result.contentHash;
      this.sourceStatus = 'ready';
    } catch (error) {
      if (request !== this.sourceRequest) return;
      source.status = 'unreadable';
      this.sourceReadStatus = 'unreadable';
      this.sourceStatus = 'error';
      this.sourceError = discoveryErrorMessage(error);
    }
  }

  private async refreshRelatedSources() {
    const project = this.selectedProject;
    const flow = this.selectedFlow;
    const request = ++this.relatedSourceRequest;
    this.relatedSourceRefs = [];
    if (!project || !flow || flow.sourceRefs.length === 0) return;

    const evidencePaths = new Set(flow.sourceRefs.map((source) => source.path));
    let relatedPaths: string[];
    if (!isTauriRuntime()) {
      relatedPaths = Object.keys(flow.files ?? {}).filter((path) => !evidencePaths.has(path));
    } else {
      try {
        relatedPaths = await listCodegraphRelatedFiles(project.id, [...evidencePaths]);
      } catch {
        relatedPaths = [];
      }
    }
    if (request !== this.relatedSourceRequest) return;

    this.relatedSourceRefs = relatedPaths.map((path) => ({
      id: `codegraph:${path}`,
      path,
      startLine: 1,
      endLine: 1,
      symbol: '',
      label: 'Related by CodeGraph',
      status: 'current',
      contentHash: '',
      origin: 'codegraph',
    }));

    if (
      this.selectedSourceRefId.startsWith('codegraph:') &&
      this.relatedSourceRefs.some((source) => source.id === this.selectedSourceRefId)
    ) {
      void this.loadSelectedSource();
    }
  }

  private applyPendingSelection() {
    const project =
      this.projects.find((item) => item.id === this.pendingSelection.projectId) ??
      this.selectedProject ??
      this.projects[0];
    if (!project) {
      this.persistViewState();
      return;
    }
    this.selectedProjectId = project.id;
    const module = findModule(project, this.pendingSelection.moduleId ?? '');
    const fallback = findFirstFlow(project);
    this.selectedModuleId = module?.id ?? fallback?.module.id ?? project.modules[0]?.id ?? '';
    const selectedModule = findModule(project, this.selectedModuleId);
    const flow = selectedModule?.flows.find((item) => item.id === this.pendingSelection.flowId);
    this.selectedFlowId = flow?.id ?? selectedModule?.flows[0]?.id ?? '';
    const selectedFlow = selectedModule?.flows.find((item) => item.id === this.selectedFlowId);
    const requestedSourceRefId = this.pendingSelection.sourceRefId;
    const source = selectedFlow?.sourceRefs.find(
      (item) => item.id === this.pendingSelection.sourceRefId,
    );
    this.selectedSourceRefId =
      source?.id ??
      (requestedSourceRefId?.startsWith('codegraph:') ? requestedSourceRefId : undefined) ??
      selectedFlow?.sourceRefs[0]?.id ??
      '';
    this.relatedSourceRefs = [];
    this.pendingSelection = {};
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
  }

  private persistViewState() {
    if (!browser) return;
    const url = new URL(window.location.href);
    for (const [key, value] of [
      ['project', this.selectedProjectId],
      ['module', this.selectedModuleId],
      ['flow', this.selectedFlowId],
      ['file', this.selectedSourceRefId],
    ]) {
      if (value) url.searchParams.set(key, value);
      else url.searchParams.delete(key);
    }
    window.cancelAnimationFrame(this.viewStateFrame);
    this.viewStateFrame = window.requestAnimationFrame(() => replaceRouteState(url, {}));
  }
}

export const inspectorStore = new InspectorStore();

function formatRelativeActivity(value: string): string {
  const timestamp = Number(value);
  if (!Number.isFinite(timestamp) || timestamp <= 0) return 'Not inspected';
  const elapsed = Math.max(0, Date.now() - timestamp);
  if (elapsed < 60_000) return 'Just now';
  if (elapsed < 3_600_000) return `${Math.floor(elapsed / 60_000)}m ago`;
  if (elapsed < 86_400_000) return `${Math.floor(elapsed / 3_600_000)}h ago`;
  return `${Math.floor(elapsed / 86_400_000)}d ago`;
}

function previewSourceFile(code: string, startLine: number, endLine: number): string {
  const paddedStart = startLine > 1 ? `${'\n'.repeat(startLine - 1)}${code}` : code;
  const missingLines = Math.max(0, endLine - paddedStart.split('\n').length);
  return missingLines > 0 ? `${paddedStart}${'\n'.repeat(missingLines)}` : paddedStart;
}
