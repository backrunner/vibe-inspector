use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EntityStatus {
    Draft,
    Ready,
    Stale,
    NeedsReview,
    Archived,
}

impl EntityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::NeedsReview => "needs-review",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "ready" => Some(Self::Ready),
            "stale" => Some(Self::Stale),
            "needs-review" => Some(Self::NeedsReview),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSnapshot {
    pub project_id: String,
    pub project_revision: u64,
    pub modules: Vec<InspectorModule>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorModule {
    pub id: String,
    pub project_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: String,
    pub summary: String,
    pub status: EntityStatus,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub flows: Vec<InspectorFlow>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorFlow {
    pub id: String,
    pub project_id: String,
    pub module_id: String,
    pub name: String,
    pub summary: String,
    pub status: EntityStatus,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
    pub graph: InspectorGraph,
    pub explanation: Option<InspectorExplanation>,
    pub source_refs: Vec<InspectorSourceRef>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorGraph {
    #[serde(default)]
    pub kind: DiagramKind,
    pub nodes: Vec<InspectorFlowNode>,
    pub edges: Vec<InspectorFlowEdge>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DiagramKind {
    Architecture,
    #[default]
    Workflow,
    Sequence,
    Dataflow,
    Lifecycle,
}

impl DiagramKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Workflow => "workflow",
            Self::Sequence => "sequence",
            Self::Dataflow => "dataflow",
            Self::Lifecycle => "lifecycle",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "architecture" => Some(Self::Architecture),
            "workflow" => Some(Self::Workflow),
            "sequence" => Some(Self::Sequence),
            "dataflow" => Some(Self::Dataflow),
            "lifecycle" => Some(Self::Lifecycle),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorFlowNode {
    pub id: String,
    pub label: String,
    pub kind: FlowNodeKind,
    pub description: Option<String>,
    pub source_ref_ids: Vec<String>,
    pub position: i64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FlowNodeKind {
    Start,
    Action,
    Decision,
    External,
    End,
}

impl FlowNodeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Action => "action",
            Self::Decision => "decision",
            Self::External => "external",
            Self::End => "end",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "action" => Some(Self::Action),
            "decision" => Some(Self::Decision),
            "external" => Some(Self::External),
            "end" => Some(Self::End),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorFlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default)]
    pub kind: FlowEdgeKind,
    pub mechanism: Option<String>,
    #[serde(default)]
    pub source_ref_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FlowEdgeKind {
    #[default]
    Control,
    Data,
    Async,
    Error,
    Dependency,
}

impl FlowEdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Data => "data",
            Self::Async => "async",
            Self::Error => "error",
            Self::Dependency => "dependency",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "control" => Some(Self::Control),
            "data" => Some(Self::Data),
            "async" => Some(Self::Async),
            "error" => Some(Self::Error),
            "dependency" => Some(Self::Dependency),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorSourceRef {
    pub id: String,
    pub project_id: String,
    pub flow_id: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub start_column: Option<usize>,
    pub end_column: Option<usize>,
    pub symbol: Option<String>,
    pub label: Option<String>,
    pub content_hash: String,
    pub status: SourceRefStatus,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SourceRefStatus {
    Current,
    Stale,
    Unreadable,
    Archived,
}

impl SourceRefStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Stale => "stale",
            Self::Unreadable => "unreadable",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "current" => Some(Self::Current),
            "stale" => Some(Self::Stale),
            "unreadable" => Some(Self::Unreadable),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectorExplanation {
    pub id: String,
    pub project_id: String,
    pub flow_id: String,
    pub headline: String,
    pub body: String,
    pub decisions: Vec<String>,
    pub confidence: Option<f64>,
    pub provenance: Provenance,
    pub revision: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub source: ProvenanceSource,
    pub conversation_id: Option<String>,
    pub model: Option<String>,
    pub prompt_hash: Option<String>,
    pub generated_at: String,
    pub confidence: Option<f64>,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ProvenanceSource {
    InspectorInit,
    InspectorSync,
    InspectorClarify,
    Mcp,
    Manual,
}

impl ProvenanceSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InspectorInit => "inspector-init",
            Self::InspectorSync => "inspector-sync",
            Self::InspectorClarify => "inspector-clarify",
            Self::Mcp => "mcp",
            Self::Manual => "manual",
        }
    }
}
