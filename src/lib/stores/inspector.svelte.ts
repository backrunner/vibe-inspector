import { browser } from '$app/environment';
import { replaceState as replaceRouteState } from '$app/navigation';
import {
  findFirstFlow,
  findModule,
  flattenModules,
  seedProjects,
  type EntityVersion,
  type EntityVersionSummary,
  type Flow,
  type Issue,
  type IssueStatus,
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
import {
  formatRelativeActivity,
  loadInspectorActivity,
  type CommandRun,
  type McpAuditEvent,
} from '$lib/adapters/activity';
import { getEntityVersion, listEntityVersions } from '$lib/adapters/versions';
import {
  listProjectIssues,
  setIssueStatus as setIssueStatusCommand,
} from '$lib/adapters/issues';
import { loadRuntimeHealth, updateLoginItem, type RuntimeHealth } from '$lib/adapters/runtime';
import {
  detectAgentIntegrations,
  installAgentIntegrations,
  type AgentIntegration,
} from '$lib/adapters/agents';
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
  projectSearchQuery = $state('');
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
  historyOpen = $state(false);
  historyEntityType = $state<'module' | 'flow'>('module');
  historyVersions = $state<EntityVersionSummary[]>([]);
  historyStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  historyError = $state('');
  selectedVersionId = $state('');
  selectedVersion = $state<EntityVersion | null>(null);
  versionDetailStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  versionDetailError = $state('');
  issuesOpen = $state(false);
  issues = $state<Issue[]>([]);
  issuesStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  issuesError = $state('');
  openIssueCount = $state(0);
  runtimeHealthStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  runtimeHealthError = $state('');
  runtimeHealth = $state<RuntimeHealth>();
  loginItemUpdating = $state(false);
  agentIntegrations = $state<AgentIntegration[]>([]);
  agentIntegrationsStatus = $state<'idle' | 'loading' | 'ready' | 'error'>('idle');
  agentSetupDismissed = $state(false);
  agentInstallUpdating = $state(false);
  agentInstallError = $state('');
  agentDetecting = $state(false);
  private inspectorRequest = 0;
  private sourceRequest = 0;
  private relatedSourceRequest = 0;
  private historyRequest = 0;
  private versionDetailRequest = 0;
  private issuesRequest = 0;
  private issueCountRequest = 0;
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

  get historyEntityName(): string {
    return this.historyEntityType === 'flow'
      ? (this.selectedFlow?.name ?? '')
      : (this.selectedModule?.name ?? '');
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
    // Re-selecting the active project is a no-op: keep the current module,
    // flow, and source selection instead of resetting and re-hydrating. After
    // a hydration error the click must still work as the retry path.
    if (projectId === this.selectedProjectId && this.inspectorStatus !== 'error') return;
    const project = this.projects.find((item) => item.id === projectId);
    if (!project) return;
    this.selectedProjectId = project.id;
    this.selectedModuleId = '';
    this.selectedFlowId = '';
    this.selectedSourceRefId = '';
    this.relatedSourceRefs = [];
    this.sourceCode = '';
    this.sourceStatus = 'idle';
    this.resetInspectorPanels();
    this.openIssueCount = 0;
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
    this.agentSetupDismissed = localStorage.getItem('vibe-inspector.agent-setup') === 'dismissed';
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

  async openHistory() {
    if (!this.selectedModule) return;
    this.historyOpen = true;
    this.historyEntityType = this.selectedFlow ? 'flow' : 'module';
    await this.loadHistoryVersions();
  }

  closeHistory() {
    this.historyOpen = false;
  }

  async setHistoryEntityType(entityType: 'module' | 'flow') {
    if (entityType === this.historyEntityType || !this.historyOpen) return;
    this.historyEntityType = entityType;
    await this.loadHistoryVersions();
  }

  async selectVersion(versionId: string) {
    const project = this.selectedProject;
    this.selectedVersionId = versionId;
    const request = ++this.versionDetailRequest;
    this.versionDetailError = '';
    if (!project || !isTauriRuntime()) {
      this.selectedVersion = null;
      this.versionDetailStatus = 'idle';
      return;
    }
    this.versionDetailStatus = 'loading';
    try {
      const version = await getEntityVersion(project.id, versionId);
      if (request !== this.versionDetailRequest) return;
      this.selectedVersion = version;
      this.versionDetailStatus = 'ready';
    } catch (error) {
      if (request !== this.versionDetailRequest) return;
      this.selectedVersion = null;
      this.versionDetailStatus = 'error';
      this.versionDetailError = discoveryErrorMessage(error);
    }
  }

  private async loadHistoryVersions() {
    const project = this.selectedProject;
    const entityId =
      this.historyEntityType === 'flow' ? this.selectedFlow?.id : this.selectedModule?.id;
    const request = ++this.historyRequest;
    this.historyError = '';
    this.selectedVersionId = '';
    this.selectedVersion = null;
    this.versionDetailStatus = 'idle';
    this.versionDetailError = '';
    if (!project || !entityId || !isTauriRuntime()) {
      this.historyVersions = [];
      this.historyStatus = 'ready';
      return;
    }
    this.historyStatus = 'loading';
    try {
      const versions = await listEntityVersions(project.id, {
        entityType: this.historyEntityType,
        entityId,
      });
      if (request !== this.historyRequest) return;
      this.historyVersions = versions;
      this.historyStatus = 'ready';
      if (versions[0]) void this.selectVersion(versions[0].id);
    } catch (error) {
      if (request !== this.historyRequest) return;
      this.historyStatus = 'error';
      this.historyError = discoveryErrorMessage(error);
    }
  }

  async openIssues() {
    if (!this.selectedModule) return;
    this.issuesOpen = true;
    await this.loadIssues();
  }

  closeIssues() {
    this.issuesOpen = false;
  }

  private async loadIssues() {
    const project = this.selectedProject;
    const module = this.selectedModule;
    const request = ++this.issuesRequest;
    this.issuesError = '';
    if (!project || !module || !isTauriRuntime()) {
      this.issues = [];
      this.issuesStatus = 'ready';
      return;
    }
    this.issuesStatus = 'loading';
    try {
      const issues = await listProjectIssues(project.id, { moduleId: module.id });
      if (request !== this.issuesRequest) return;
      this.issues = issues;
      this.issuesStatus = 'ready';
    } catch (error) {
      if (request !== this.issuesRequest) return;
      this.issuesStatus = 'error';
      this.issuesError = discoveryErrorMessage(error);
    }
  }

  async setIssueStatus(issue: Issue, status: IssueStatus) {
    const project = this.selectedProject;
    if (!project || issue.status === status) return;
    this.issuesError = '';
    try {
      const updated = await setIssueStatusCommand(project.id, issue, status);
      this.issues = this.issues.map((item) => (item.id === updated.id ? updated : item));
      const delta = issue.status === 'open' ? -1 : status === 'open' ? 1 : 0;
      this.openIssueCount = Math.max(0, this.openIssueCount + delta);
    } catch (error) {
      this.issuesError = discoveryErrorMessage(error);
    }
  }

  private async refreshOpenIssueCount() {
    const project = this.selectedProject;
    const module = this.selectedModule;
    const request = ++this.issueCountRequest;
    if (!project || !module || !isTauriRuntime()) {
      this.openIssueCount = 0;
      return;
    }
    try {
      const open = await listProjectIssues(project.id, { moduleId: module.id, status: 'open' });
      if (request !== this.issueCountRequest) return;
      this.openIssueCount = open.length;
    } catch {
      if (request !== this.issueCountRequest) return;
      this.openIssueCount = 0;
    }
  }

  private resetInspectorPanels() {
    this.historyRequest += 1;
    this.versionDetailRequest += 1;
    this.issuesRequest += 1;
    this.historyOpen = false;
    this.historyVersions = [];
    this.historyStatus = 'idle';
    this.historyError = '';
    this.selectedVersionId = '';
    this.selectedVersion = null;
    this.versionDetailStatus = 'idle';
    this.versionDetailError = '';
    this.issuesOpen = false;
    this.issues = [];
    this.issuesStatus = 'idle';
    this.issuesError = '';
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

  async detectAgents() {
    if (this.agentDetecting) return;
    // Re-detection happens on every view mount so newly installed agents show
    // up immediately; with existing data we refresh silently in the background
    // instead of blanking the list.
    const hasData = this.agentIntegrations.length > 0;
    this.agentDetecting = true;
    if (!hasData) this.agentIntegrationsStatus = 'loading';
    try {
      this.agentIntegrations = await detectAgentIntegrations();
      this.agentIntegrationsStatus = 'ready';
    } catch {
      if (!hasData) this.agentIntegrationsStatus = 'error';
    } finally {
      this.agentDetecting = false;
    }
  }

  async installAgents(agentIds: string[]) {
    if (agentIds.length === 0 || this.agentInstallUpdating) return;
    this.agentInstallUpdating = true;
    this.agentInstallError = '';
    try {
      const results = await installAgentIntegrations(agentIds);
      for (const result of results) {
        if (!result.integration) continue;
        const index = this.agentIntegrations.findIndex((entry) => entry.id === result.agentId);
        if (index >= 0) this.agentIntegrations[index] = result.integration;
        else this.agentIntegrations.push(result.integration);
      }
      const failures = results.filter((result) => result.error);
      if (failures.length > 0) {
        this.agentInstallError = failures
          .map((failure) => `${failure.agentId}: ${failure.error?.message ?? 'install failed'}`)
          .join('; ');
      }
      this.agentIntegrationsStatus = 'ready';
    } catch (error) {
      // Keep showing the current list when a refresh/install round-trip fails;
      // only the very first load without data drops into the error state.
      if (this.agentIntegrations.length === 0) this.agentIntegrationsStatus = 'error';
      this.agentInstallError = discoveryErrorMessage(error);
    } finally {
      this.agentInstallUpdating = false;
    }
  }

  dismissAgentSetup() {
    this.agentSetupDismissed = true;
    if (browser) localStorage.setItem('vibe-inspector.agent-setup', 'dismissed');
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
    this.resetInspectorPanels();
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
    void this.refreshOpenIssueCount();
  }

  selectFlow(moduleId: string, flowId: string) {
    const module = findModule(this.selectedProject, moduleId);
    const flow = module?.flows.find((item) => item.id === flowId);
    if (!module || !flow) return;
    const moduleChanged = module.id !== this.selectedModuleId;
    this.selectedModuleId = module.id;
    this.selectedFlowId = flow.id;
    this.selectedSourceRefId = flow.sourceRefs[0]?.id ?? '';
    this.relatedSourceRefs = [];
    this.resetInspectorPanels();
    this.persistViewState();
    void this.loadSelectedSource();
    void this.refreshRelatedSources();
    if (moduleChanged) void this.refreshOpenIssueCount();
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
    void this.refreshOpenIssueCount();
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
    let relatedFiles: Array<{
      path: string;
      relationship?: SourceRef['codegraphRelationship'];
      kinds?: string[];
    }>;
    if (!isTauriRuntime()) {
      relatedFiles = Object.keys(flow.files ?? {})
        .filter((path) => !evidencePaths.has(path))
        .map((path) => ({ path }));
    } else {
      try {
        relatedFiles = await listCodegraphRelatedFiles(project.id, [...evidencePaths]);
      } catch {
        relatedFiles = [];
      }
    }
    if (request !== this.relatedSourceRequest) return;

    this.relatedSourceRefs = relatedFiles.map((relation) => ({
      id: `codegraph:${relation.path}`,
      path: relation.path,
      startLine: 1,
      endLine: 1,
      symbol: '',
      label: 'Related by CodeGraph',
      status: 'current',
      contentHash: '',
      origin: 'codegraph',
      codegraphRelationship: relation.relationship,
      codegraphKinds: relation.kinds,
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
    void this.refreshOpenIssueCount();
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

function previewSourceFile(code: string, startLine: number, endLine: number): string {
  const paddedStart = startLine > 1 ? `${'\n'.repeat(startLine - 1)}${code}` : code;
  const missingLines = Math.max(0, endLine - paddedStart.split('\n').length);
  return missingLines > 0 ? `${paddedStart}${'\n'.repeat(missingLines)}` : paddedStart;
}
