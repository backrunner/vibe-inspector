export type ProjectStatus = 'ready' | 'stale' | 'syncing' | 'error';
export type EntityStatus = 'draft' | 'ready' | 'stale' | 'needs-review' | 'archived';

export type SourceRef = {
  id: string;
  path: string;
  startLine: number;
  endLine: number;
  symbol: string;
  label: string;
  status: 'current' | 'stale' | 'unreadable';
  contentHash: string;
  origin?: 'evidence' | 'codegraph';
  codegraphRelationship?: 'dependency' | 'dependent' | 'both';
  codegraphKinds?: string[];
  revision?: number;
};

export type FlowNodeKind = 'start' | 'action' | 'decision' | 'external' | 'end';

export type FlowNode = {
  id: string;
  label: string;
  kind: FlowNodeKind;
  description: string;
  sourceRefIds: string[];
  x: number;
  y: number;
};

export type FlowEdge = {
  id: string;
  from: string;
  to: string;
  label?: string;
};

export type Provenance = {
  source: 'inspector-init' | 'inspector-sync' | 'inspector-clarify' | 'mcp' | 'manual';
  conversationId?: string;
  model?: string;
  promptHash?: string;
  generatedAt: string;
  confidence?: number;
  evidenceIds: string[];
};

export type Flow = {
  id: string;
  name: string;
  summary: string;
  status: EntityStatus;
  revision?: number;
  owner: string;
  updatedAt: string;
  graph: { nodes: FlowNode[]; edges: FlowEdge[] };
  explanation: {
    headline: string;
    body: string;
    decisions: string[];
    confidence: number;
    provenance: Provenance;
  } | null;
  sourceRefs: SourceRef[];
  files?: Record<string, string>;
};

export type Module = {
  id: string;
  name: string;
  slug: string;
  summary: string;
  status: EntityStatus;
  revision?: number;
  flowCount: number;
  children?: Module[];
  flows: Flow[];
};

export type Project = {
  id: string;
  name: string;
  rootPath: string;
  status: ProjectStatus;
  branch: string;
  lastScanAt: string;
  modules: Module[];
  source: 'codex-history' | 'manual';
  conversationCount: number;
  revision?: number;
};

const checkoutCode = `import { reserveInventory } from '$lib/inventory';
import { createPaymentIntent } from '$lib/payments';
import { orderRepository } from '$lib/orders';

export async function checkout(input: CheckoutInput) {
  const reservation = await reserveInventory(input.items);

  if (!reservation.available) {
    return { ok: false, reason: 'inventory_unavailable' };
  }

  const payment = await createPaymentIntent({
    amount: input.total,
    currency: input.currency,
    idempotencyKey: input.requestId,
  });

  if (!payment.approved) {
    await reservation.release();
    return { ok: false, reason: 'payment_declined' };
  }

  const order = await orderRepository.create({
    customerId: input.customerId,
    reservationId: reservation.id,
    paymentId: payment.id,
  });

  return { ok: true, orderId: order.id };
}`;

const paymentCode = `export async function createPaymentIntent(input: PaymentIntentInput) {
  const existing = await paymentStore.findByIdempotencyKey(input.idempotencyKey);
  if (existing) return existing;

  const intent = await provider.paymentIntents.create({
    amount: input.amount,
    currency: input.currency,
    metadata: { requestId: input.idempotencyKey },
  });

  return paymentStore.insert({
    providerId: intent.id,
    status: intent.status,
    idempotencyKey: input.idempotencyKey,
  });
}`;

const streamCode = `export async function consumeOrderEvents(batch: EventBatch) {
  const checkpoint = await checkpoints.read(batch.partition);
  const events = batch.events.filter((event) => event.offset > checkpoint);

  for (const event of events) {
    await projection.apply(event);
    await checkpoints.write(batch.partition, event.offset);
  }

  return { processed: events.length, checkpoint: batch.events.at(-1)?.offset ?? checkpoint };
}`;

const inspectorCommandCode = `pub fn run_inspector_command(
    database: &InspectorDatabase,
    input: RunInspectorCommandInput,
    actor: &str,
) -> Result<RunInspectorCommandResult, AppError> {
    validate_request(&input, actor)?;
    if let Some(stored) = database.command_run(&input.request_id)? {
        if stored.project_id != input.project_id || stored.command != input.command.as_str() {
            return Err(AppError::Validation(format!(
                "requestId {} was already used for another command",
                input.request_id
            )));
        }
        let result = stored
            .result_json
            .ok_or_else(|| AppError::Database("command result is missing".to_string()))?;
        return serde_json::from_str(&result)
            .map_err(|error| AppError::Database(error.to_string()));
    }

    let current = database.load_inspector_snapshot(&input.project_id)?;
    if current.project_revision != input.expected_revision {
        return Err(AppError::RevisionConflict {
            entity_id: input.project_id.clone(),
            expected: input.expected_revision,
            actual: current.project_revision,
        });
    }
    validate_scope(&current, &input)?;
    if input.command == InspectorCommand::InspectorSync {
        validate_stable_ids(&current, &input.proposal)?;
    }
    if matches!(
        input.command,
        InspectorCommand::InspectorSync | InspectorCommand::InspectorClarify
    ) {
        ensure_scope_unchanged(&current, &input.proposal, &input.scope)?;
    }

    let scope_fingerprint = scope_fingerprint(&input)?;
    let structural_summary = analyze_structure(&current, &input.proposal);
    let review_reason = review_reason(&input, &structural_summary);`;

const inspectorNormalizeCode = `fn apply_explanation_metadata(
    explanation: &mut InspectorExplanation,
    current: Option<&InspectorExplanation>,
    input: &RunInspectorCommandInput,
    now: &str,
) -> Result<(), AppError> {
    let changed = current.is_none_or(|current| !explanation_content_eq(current, explanation));
    if changed {
        explanation.provenance.source = input.command.provenance_source();
        explanation.provenance.conversation_id = input.conversation_id.clone();
        explanation.provenance.model = input.model.clone();
        explanation.provenance.prompt_hash = input.prompt_hash.clone();
        explanation.provenance.generated_at = now.to_string();
    }
    if let Some(current) = current {
        explanation.created_at = current.created_at.clone();
        explanation.revision = next_revision(current.revision, changed)?;`;

const inspectorAdapterCode = `export function snapshotModules(snapshot: InspectorSnapshotDto): Module[] {
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
      },`;

const sourceTreeCode = `import type { SourceRef } from './entities';

export type SourceTreeFile = {
  kind: 'file';
  id: string;
  name: string;
  path: string;
  refs: SourceRef[];
};

export function buildSourceTree(sourceRefs: SourceRef[]): SourceTreeNode[] {
  const root = createFolder('', '');
  for (const sourceRef of sourceRefs) {
    insertSourceReference(root, sourceRef);
  }
  return materialize(root);
}

export function sourceTreeFileCount(nodes: SourceTreeNode[]): number {
  return nodes.reduce(
    (count, node) => count + (node.kind === 'file' ? 1 : sourceTreeFileCount(node.children)),
    0,
  );
}`;

const editorAdapterCode = `export function readonlyEditorOptions(input: ReadonlyEditorOptionsInput) {
  return {
    value: input.value,
    language: languageForPath(input.path),
    theme: input.theme,
    readOnly: true,
    domReadOnly: true,
    contextmenu: false,
    quickSuggestions: false,
    minimap: { enabled: false },
    lineNumbers: (lineNumber: number) => String(lineNumber + input.codeStartLine - 1),
    lineNumbersMinChars: lineNumberMinChars(input.value, input.codeStartLine),
    glyphMargin: false,
    folding: true,
    wordWrap: 'off',
    scrollBeyondLastLine: false,
    automaticLayout: true,
  };
}`;

const checkoutFlow: Flow = {
  id: 'flow-checkout',
  name: 'Checkout and payment authorization',
  summary:
    'Reserves inventory, authorizes payment, and persists an order with retry-safe semantics.',
  status: 'ready',
  owner: 'Commerce Platform',
  updatedAt: '12 min ago',
  graph: {
    nodes: [
      {
        id: 'start',
        label: 'Checkout request',
        kind: 'start',
        description: 'Customer submits a cart.',
        sourceRefIds: ['ref-checkout'],
        x: 48,
        y: 56,
      },
      {
        id: 'reserve',
        label: 'Reserve inventory',
        kind: 'action',
        description: 'Create a short-lived reservation.',
        sourceRefIds: ['ref-checkout'],
        x: 250,
        y: 56,
      },
      {
        id: 'available',
        label: 'Items available?',
        kind: 'decision',
        description: 'Stop before charging if stock changed.',
        sourceRefIds: ['ref-checkout'],
        x: 482,
        y: 56,
      },
      {
        id: 'payment',
        label: 'Authorize payment',
        kind: 'external',
        description: 'Create or reuse an idempotent intent.',
        sourceRefIds: ['ref-payment'],
        x: 718,
        y: 56,
      },
      {
        id: 'persist',
        label: 'Persist order',
        kind: 'action',
        description: 'Write the order after approval.',
        sourceRefIds: ['ref-checkout'],
        x: 948,
        y: 56,
      },
      {
        id: 'decline',
        label: 'Release + explain',
        kind: 'end',
        description: 'Return a typed failure reason.',
        sourceRefIds: ['ref-checkout'],
        x: 718,
        y: 210,
      },
      {
        id: 'done',
        label: 'Order confirmed',
        kind: 'end',
        description: 'Return an order ID to the caller.',
        sourceRefIds: ['ref-checkout'],
        x: 948,
        y: 210,
      },
    ],
    edges: [
      { id: 'e1', from: 'start', to: 'reserve' },
      { id: 'e2', from: 'reserve', to: 'available' },
      { id: 'e3', from: 'available', to: 'payment', label: 'yes' },
      { id: 'e4', from: 'available', to: 'decline', label: 'no' },
      { id: 'e5', from: 'payment', to: 'persist', label: 'approved' },
      { id: 'e6', from: 'payment', to: 'decline', label: 'declined' },
      { id: 'e7', from: 'persist', to: 'done' },
    ],
  },
  explanation: {
    headline: 'Reserve before charge; persist only after approval.',
    body: 'The flow makes the irreversible payment side effect the middle boundary. Inventory is reserved first and released on decline, so a failed payment cannot leave stock locked. The order write happens after the provider response and receives the same request identity for safe retries.',
    decisions: [
      'Inventory is a precondition, not a compensating action.',
      'Payment intent keys are reused across retries.',
      'Source references point to the exact branch decisions.',
    ],
    confidence: 0.94,
    provenance: {
      source: 'inspector-init',
      conversationId: 'conv-commerce-review',
      model: 'gpt-5',
      generatedAt: '2026-07-14T02:42:00Z',
      confidence: 0.94,
      evidenceIds: ['ref-checkout', 'ref-payment'],
    },
  },
  sourceRefs: [
    {
      id: 'ref-checkout',
      path: 'apps/checkout/src/checkout.ts',
      startLine: 5,
      endLine: 29,
      symbol: 'checkout',
      label: 'Orchestration and failure branches',
      status: 'current',
      contentHash: '6e2a4d8',
    },
    {
      id: 'ref-payment',
      path: 'packages/payments/src/create-intent.ts',
      startLine: 1,
      endLine: 18,
      symbol: 'createPaymentIntent',
      label: 'Idempotent provider boundary',
      status: 'current',
      contentHash: 'b19f0f2',
    },
  ],
  files: {
    'apps/checkout/src/checkout.ts': checkoutCode,
    'packages/payments/src/create-intent.ts': paymentCode,
  },
};

const orderStreamFlow: Flow = {
  id: 'flow-order-stream',
  name: 'Order event projection',
  summary: 'Applies ordered events to a read model with a durable partition checkpoint.',
  status: 'needs-review',
  owner: 'Data Platform',
  updatedAt: '3 hours ago',
  graph: {
    nodes: [
      {
        id: 'consume',
        label: 'Consume batch',
        kind: 'start',
        description: 'Read events from a partition.',
        sourceRefIds: ['ref-stream'],
        x: 130,
        y: 90,
      },
      {
        id: 'checkpoint',
        label: 'Load checkpoint',
        kind: 'action',
        description: 'Ignore events already applied.',
        sourceRefIds: ['ref-stream'],
        x: 370,
        y: 90,
      },
      {
        id: 'project',
        label: 'Apply projection',
        kind: 'action',
        description: 'Update the read model.',
        sourceRefIds: ['ref-stream'],
        x: 610,
        y: 90,
      },
      {
        id: 'save',
        label: 'Save offset',
        kind: 'end',
        description: 'Resume from the last durable event.',
        sourceRefIds: ['ref-stream'],
        x: 850,
        y: 90,
      },
    ],
    edges: [
      { id: 's1', from: 'consume', to: 'checkpoint' },
      { id: 's2', from: 'checkpoint', to: 'project' },
      { id: 's3', from: 'project', to: 'save' },
    ],
  },
  explanation: {
    headline: 'The checkpoint is the correctness boundary.',
    body: 'Projection work is intentionally sequenced with checkpoint writes. This favors at-least-once delivery and makes a crash replayable. The review flag is kept because the source writes one checkpoint per event; batching that write is a possible throughput improvement, but it would change recovery semantics.',
    decisions: [
      'Replay is safe because events are filtered by offset.',
      'Checkpoint writes follow projection writes.',
      'The open review item is a throughput tradeoff, not a correctness bug.',
    ],
    confidence: 0.88,
    provenance: {
      source: 'inspector-sync',
      conversationId: 'conv-order-projection',
      model: 'gpt-5',
      generatedAt: '2026-07-13T23:18:00Z',
      confidence: 0.88,
      evidenceIds: ['ref-stream'],
    },
  },
  sourceRefs: [
    {
      id: 'ref-stream',
      path: 'services/events/src/consume-orders.ts',
      startLine: 1,
      endLine: 14,
      symbol: 'consumeOrderEvents',
      label: 'Partition replay and checkpoint',
      status: 'current',
      contentHash: 'aa44e93',
    },
  ],
  files: { 'services/events/src/consume-orders.ts': streamCode },
};

const inspectorFlow: Flow = {
  id: 'flow-inspector-refresh',
  name: 'Inspector refresh proposal',
  summary:
    'Scans a repository snapshot and reconciles an AI proposal without discarding stable IDs.',
  status: 'draft',
  owner: 'Developer Experience',
  updatedAt: 'Yesterday',
  graph: {
    nodes: [
      {
        id: 'scope',
        label: 'Resolve scope',
        kind: 'start',
        description: 'Use the selected module or conversation context.',
        sourceRefIds: ['ref-inspector-command'],
        x: 110,
        y: 90,
      },
      {
        id: 'scan',
        label: 'Scan snapshot',
        kind: 'action',
        description: 'Read files and symbols within the project root.',
        sourceRefIds: ['ref-inspector-command'],
        x: 360,
        y: 90,
      },
      {
        id: 'propose',
        label: 'Propose changes',
        kind: 'external',
        description: 'Ask the analyst to preserve existing IDs.',
        sourceRefIds: ['ref-inspector-normalize'],
        x: 610,
        y: 90,
      },
      {
        id: 'reconcile',
        label: 'Review proposal',
        kind: 'decision',
        description: 'Separate content updates from structural changes.',
        sourceRefIds: ['ref-inspector-command', 'ref-inspector-adapter'],
        x: 860,
        y: 90,
      },
    ],
    edges: [
      { id: 'i1', from: 'scope', to: 'scan' },
      { id: 'i2', from: 'scan', to: 'propose' },
      { id: 'i3', from: 'propose', to: 'reconcile' },
    ],
  },
  explanation: {
    headline: 'Sync is a reconciliation, not a rebuild.',
    body: 'The refresh path separates repository evidence from the analyst proposal. A structural change is surfaced as a reviewable diff, while explanations and source hashes can update without changing the navigation tree.',
    decisions: [
      'Stable IDs survive refreshes.',
      'Structural changes require explicit review.',
      'The UI can show syncing without losing selection.',
    ],
    confidence: 0.9,
    provenance: {
      source: 'inspector-sync',
      conversationId: 'conv-vibe-inspector-refresh',
      model: 'gpt-5',
      promptHash: 'sha256:2a41b9c5',
      generatedAt: '2026-07-14T02:42:00Z',
      confidence: 0.9,
      evidenceIds: [
        'ref-inspector-command',
        'ref-inspector-normalize',
        'ref-inspector-adapter',
      ],
    },
  },
  sourceRefs: [
    {
      id: 'ref-inspector-command',
      path: 'src-tauri/src/command_service/mod.rs',
      startLine: 141,
      endLine: 183,
      symbol: 'run_inspector_command',
      label: 'Idempotency, revision, and structural review boundary',
      status: 'current',
      contentHash: 'a93d2f1',
    },
    {
      id: 'ref-inspector-normalize',
      path: 'src-tauri/src/command_service/normalize.rs',
      startLine: 507,
      endLine: 525,
      symbol: 'apply_explanation_metadata',
      label: 'Provenance and revision preservation',
      status: 'current',
      contentHash: '74c10be',
    },
    {
      id: 'ref-inspector-adapter',
      path: 'src/lib/adapters/inspector.ts',
      startLine: 88,
      endLine: 118,
      symbol: 'snapshotModules',
      label: 'Typed snapshot mapping into UI state',
      status: 'current',
      contentHash: '5c219d7',
    },
  ],
  files: {
    'src-tauri/src/command_service/mod.rs': inspectorCommandCode,
    'src-tauri/src/command_service/normalize.rs': inspectorNormalizeCode,
    'src/lib/adapters/inspector.ts': inspectorAdapterCode,
    'src/lib/adapters/editor.ts': editorAdapterCode,
    'src/lib/domain/source-tree.ts': sourceTreeCode,
  },
};

const projects: Project[] = [
  {
    id: 'project-vibe-inspector',
    name: 'vibe-inspector',
    rootPath: '/Volumes/BRData/projects/vibe-inspector',
    status: 'ready',
    branch: 'main',
    lastScanAt: 'Today, 10:42 AM',
    source: 'manual',
    conversationCount: 18,
    modules: [
      {
        id: 'module-devex',
        name: 'Developer Experience',
        slug: 'developer-experience',
        summary: 'Inspector UI, repository evidence, and Codex workflow integration.',
        status: 'ready',
        flowCount: 1,
        flows: [],
        children: [
          {
            id: 'module-inspector-ui',
            name: 'Inspector UI',
            slug: 'inspector-ui',
            summary: 'Desktop shell and inspection workspace.',
            status: 'ready',
            flowCount: 1,
            flows: [inspectorFlow],
          },
          {
            id: 'module-history',
            name: 'History Scanner',
            slug: 'history-scanner',
            summary: 'Local Codex conversation discovery.',
            status: 'draft',
            flowCount: 0,
            flows: [],
          },
        ],
      },
      {
        id: 'module-data',
        name: 'Persistence & Evidence',
        slug: 'persistence-evidence',
        summary: 'Versioned inspector entities, source hashes, and audit history.',
        status: 'draft',
        flowCount: 0,
        flows: [],
      },
    ],
  },
  {
    id: 'project-arcade-commerce',
    name: 'arcade-commerce',
    rootPath: '~/Projects/arcade-commerce',
    status: 'syncing',
    branch: 'feat/checkout-retry',
    lastScanAt: '18 min ago',
    source: 'codex-history',
    conversationCount: 42,
    modules: [
      {
        id: 'module-checkout',
        name: 'Checkout',
        slug: 'checkout',
        summary: 'Cart, payment, and order orchestration.',
        status: 'ready',
        flowCount: 2,
        flows: [checkoutFlow],
        children: [
          {
            id: 'module-payments',
            name: 'Payments',
            slug: 'payments',
            summary: 'Provider boundaries and retry policies.',
            status: 'ready',
            flowCount: 0,
            flows: [],
          },
        ],
      },
      {
        id: 'module-orders',
        name: 'Orders',
        slug: 'orders',
        summary: 'Order lifecycle and read models.',
        status: 'needs-review',
        flowCount: 1,
        flows: [orderStreamFlow],
      },
    ],
  },
  {
    id: 'project-atlas',
    name: 'atlas-monorepo',
    rootPath: '~/Work/atlas-monorepo',
    status: 'stale',
    branch: 'main',
    lastScanAt: '2 days ago',
    source: 'codex-history',
    conversationCount: 7,
    modules: [
      {
        id: 'module-platform',
        name: 'Platform',
        slug: 'platform',
        summary: 'Shared runtime packages and deployment.',
        status: 'stale',
        flowCount: 0,
        flows: [],
      },
    ],
  },
];

export const seedProjects = projects;

export function flattenModules(modules: Module[]): Module[] {
  return modules.flatMap((module) => [
    module,
    ...(module.children ? flattenModules(module.children) : []),
  ]);
}

export function findModule(
  project: Project | undefined,
  moduleId: string | undefined,
): Module | undefined {
  if (!project || !moduleId) return undefined;
  return flattenModules(project.modules).find((module) => module.id === moduleId);
}

export function findFirstFlow(
  project: Project | undefined,
): { module: Module; flow: Flow } | undefined {
  if (!project) return undefined;
  const module = flattenModules(project.modules).find((item) => item.flows.length > 0);
  const flow = module?.flows[0];
  return module && flow ? { module, flow } : undefined;
}
