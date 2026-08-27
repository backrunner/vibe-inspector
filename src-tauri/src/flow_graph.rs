use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    inspector::{FlowNodeKind, InspectorFlow, InspectorSnapshot},
};

const MAX_NODE_LABEL_CHARS: usize = 240;
const MAX_EDGE_LABEL_CHARS: usize = 160;
const MAX_EDGE_MECHANISM_CHARS: usize = 120;
const MAX_GRAPH_EDGES: usize = 2_000;
const RECOMMENDED_PRIMARY_NODES: usize = 12;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum GraphDiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiagnosticEvidence {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiagnostic {
    pub code: String,
    pub severity: GraphDiagnosticSeverity,
    pub flow_id: String,
    pub subject: String,
    pub message: String,
    pub evidence: Vec<GraphDiagnosticEvidence>,
    pub supported_fixes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphValidationReport {
    pub diagnostics: Vec<GraphDiagnostic>,
}

impl GraphValidationReport {
    pub fn single(flow_id: &str, code: &str, message: &str) -> Self {
        Self {
            diagnostics: vec![GraphDiagnostic {
                code: code.to_string(),
                severity: GraphDiagnosticSeverity::Error,
                flow_id: flow_id.to_string(),
                subject: "snapshot".to_string(),
                message: message.to_string(),
                evidence: Vec::new(),
                supported_fixes: vec!["correct the proposal and retry".to_string()],
            }],
        }
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == GraphDiagnosticSeverity::Error)
    }

    pub fn push(&mut self, diagnostic: GraphDiagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

impl fmt::Display for GraphValidationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let errors = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == GraphDiagnosticSeverity::Error)
            .count();
        let warnings = self.diagnostics.len().saturating_sub(errors);
        let first = self
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.severity == GraphDiagnosticSeverity::Error)
            .or(self.diagnostics.first())
            .map(|diagnostic| diagnostic.message.as_str())
            .unwrap_or("graph validation failed");
        write!(formatter, "{first} ({errors} errors, {warnings} warnings)")
    }
}

pub fn validate_renderable_snapshot(
    snapshot: &InspectorSnapshot,
) -> Result<GraphValidationReport, AppError> {
    let mut report = GraphValidationReport::default();
    for flow in snapshot
        .modules
        .iter()
        .flat_map(|module| module.flows.iter())
    {
        analyze_graph(flow, &mut report);
    }
    if report.has_errors() {
        return Err(AppError::RenderError(report));
    }
    Ok(report)
}

pub fn validate_renderable_flow(flow: &InspectorFlow) -> Result<GraphValidationReport, AppError> {
    let mut report = GraphValidationReport::default();
    analyze_graph(flow, &mut report);
    if report.has_errors() {
        return Err(AppError::RenderError(report));
    }
    Ok(report)
}

fn analyze_graph(flow: &InspectorFlow, report: &mut GraphValidationReport) {
    let graph = &flow.graph;
    if graph.nodes.len() < 2 {
        report.push(error(
            flow,
            "graph.nodes",
            "graph.nodes.too-few",
            "Business flows need at least two steps.",
            vec!["include at least two evidence-backed steps"],
        ));
    }
    if graph.nodes.len() > RECOMMENDED_PRIMARY_NODES {
        report.push(warning(
            flow,
            "graph.nodes",
            "graph.nodes.too-dense",
            "The graph exceeds the recommended 12 primary steps.",
            vec!["split the flow by outcome or move secondary detail into a narrower flow"],
        ));
    }
    if graph.edges.is_empty() {
        report.push(error(
            flow,
            "graph.edges",
            "graph.edges.missing",
            "Business flow steps must be connected.",
            vec!["connect the steps with at least one directed edge"],
        ));
    }
    if graph.edges.len() > MAX_GRAPH_EDGES {
        report.push(error(
            flow,
            "graph.edges",
            "graph.edges.limit",
            "The graph exceeds the 2,000 edge safety limit.",
            vec!["reduce the graph below 2,000 edges"],
        ));
    }

    let source_ids = flow
        .source_refs
        .iter()
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    let mut node_ids = HashSet::with_capacity(graph.nodes.len());
    let mut starts = Vec::new();
    let mut ends = HashSet::new();
    for node in &graph.nodes {
        if node.id.trim().is_empty() || !node_ids.insert(node.id.as_str()) {
            report.push(error(
                flow,
                &format!("graph.nodes/{}", node.id),
                "graph.node.id-invalid",
                "Every step needs a unique non-empty ID.",
                vec!["assign a stable unique ID to the named step"],
            ));
        }
        let label = node.label.trim();
        if label.is_empty() || label.chars().count() > MAX_NODE_LABEL_CHARS {
            report.push(error(
                flow,
                &format!("graph.nodes/{}", node.id),
                "graph.node.label-invalid",
                "Step labels must contain 1-240 characters.",
                vec!["replace the label with a concise business or runtime action"],
            ));
        }
        if node
            .source_ref_ids
            .iter()
            .any(|id| !source_ids.contains(id.as_str()))
        {
            report.push(error(
                flow,
                &format!("graph.nodes/{}", node.id),
                "graph.node.evidence-missing",
                "The step references evidence outside its flow.",
                vec!["attach only source reference IDs declared by this flow"],
            ));
        }
        match node.kind {
            FlowNodeKind::Start => starts.push(node.id.as_str()),
            FlowNodeKind::End => {
                ends.insert(node.id.as_str());
            }
            _ => {}
        }
    }
    if starts.is_empty() {
        report.push(error(
            flow,
            "graph.nodes",
            "graph.start.missing",
            "The graph needs at least one start step.",
            vec!["identify the concrete trigger or entrypoint and mark it as start"],
        ));
    }
    if ends.is_empty() {
        report.push(error(
            flow,
            "graph.nodes",
            "graph.end.missing",
            "The graph needs at least one end step.",
            vec!["identify the observable outcome and mark it as end"],
        ));
    }

    let mut edge_ids = HashSet::with_capacity(graph.edges.len());
    let mut endpoints = HashSet::new();
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut incoming: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut outgoing_edges: HashMap<&str, Vec<&crate::inspector::InspectorFlowEdge>> =
        HashMap::new();
    for edge in &graph.edges {
        if edge.id.trim().is_empty() || !edge_ids.insert(edge.id.as_str()) {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.id-invalid",
                "Every relationship needs a unique non-empty ID.",
                vec!["assign a stable unique ID to the named relationship"],
            ));
        }
        if edge.from == edge.to {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.self-loop",
                "A relationship cannot connect a step to itself.",
                vec!["name the distinct retry or state-transition step explicitly"],
            ));
        }
        if !endpoints.insert((edge.from.as_str(), edge.to.as_str())) {
            report.push(warning(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.duplicate-route",
                "Multiple relationships share the same direction and endpoints.",
                vec!["merge equivalent relationships or make their distinct mechanisms explicit"],
            ));
        }
        if !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()) {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.endpoint-missing",
                "Relationships may connect only steps in the same flow.",
                vec!["replace the endpoint with a step ID declared by this flow"],
            ));
            continue;
        }
        if edge
            .label
            .as_deref()
            .is_some_and(|label| label.trim().chars().count() > MAX_EDGE_LABEL_CHARS)
        {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.label-too-long",
                "Relationship labels must stay within 160 characters.",
                vec!["keep only the action, protocol, condition, or data contract in the label"],
            ));
        }
        if edge
            .mechanism
            .as_deref()
            .is_some_and(|mechanism| mechanism.trim().chars().count() > MAX_EDGE_MECHANISM_CHARS)
        {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.mechanism-too-long",
                "Relationship mechanisms must stay within 120 characters.",
                vec!["use a concise protocol, API, event, queue, or storage mechanism"],
            ));
        }
        if edge
            .source_ref_ids
            .iter()
            .any(|id| !source_ids.contains(id.as_str()))
        {
            report.push(error(
                flow,
                &format!("graph.edges/{}", edge.id),
                "graph.edge.evidence-missing",
                "The relationship references evidence outside its flow.",
                vec!["attach only source reference IDs declared by this flow"],
            ));
        }
        if edge.source_ref_ids.is_empty() {
            report.push(warning(flow, &format!("graph.edges/{}", edge.id), "graph.edge.evidence-unverified", "The relationship has no direct source evidence.", vec!["attach the call site, handler, event producer, query, or configuration that proves it"]));
        }
        outgoing
            .entry(edge.from.as_str())
            .or_default()
            .push(edge.to.as_str());
        incoming
            .entry(edge.to.as_str())
            .or_default()
            .push(edge.from.as_str());
        outgoing_edges
            .entry(edge.from.as_str())
            .or_default()
            .push(edge);
    }

    for node in &graph.nodes {
        if node.kind == FlowNodeKind::Decision {
            let branches = outgoing_edges
                .get(node.id.as_str())
                .map_or(&[][..], Vec::as_slice);
            if branches.len() < 2
                || branches
                    .iter()
                    .any(|edge| edge.label.as_deref().is_none_or(|label| label.trim().is_empty()))
            {
                report.push(error(
                    flow,
                    &format!("graph.nodes/{}", node.id),
                    "graph.decision.branches-invalid",
                    "Decision steps need at least two labeled outgoing branches.",
                    vec!["add explicit condition or outcome labels to every decision branch"],
                ));
            }
        }
    }

    if node_ids.len() == graph.nodes.len() {
        let reachable_from_start = traverse(starts, &outgoing);
        if reachable_from_start.len() != graph.nodes.len() {
            report.push(error(
                flow,
                "graph",
                "graph.path.unreachable-from-start",
                "Every step must be reachable from a start step.",
                vec!["connect orphaned steps to the nearest preceding step"],
            ));
        }
        let reaches_an_end = traverse(ends.iter().copied(), &incoming);
        if reaches_an_end.len() != graph.nodes.len() {
            report.push(error(
                flow,
                "graph",
                "graph.path.cannot-reach-end",
                "Every step must be able to reach an end step.",
                vec!["connect terminal branches to an explicit observable outcome"],
            ));
        }
    }
}

fn traverse<'a>(
    roots: impl IntoIterator<Item = &'a str>,
    adjacency: &HashMap<&'a str, Vec<&'a str>>,
) -> HashSet<&'a str> {
    let mut visited = HashSet::new();
    let mut pending = roots.into_iter().collect::<VecDeque<_>>();
    while let Some(id) = pending.pop_front() {
        if !visited.insert(id) {
            continue;
        }
        if let Some(next) = adjacency.get(id) {
            pending.extend(next.iter().copied());
        }
    }
    visited
}

fn error(
    flow: &InspectorFlow,
    subject: &str,
    code: &str,
    message: &str,
    fixes: Vec<&str>,
) -> GraphDiagnostic {
    diagnostic(
        flow,
        subject,
        code,
        message,
        fixes,
        GraphDiagnosticSeverity::Error,
    )
}

fn warning(
    flow: &InspectorFlow,
    subject: &str,
    code: &str,
    message: &str,
    fixes: Vec<&str>,
) -> GraphDiagnostic {
    diagnostic(
        flow,
        subject,
        code,
        message,
        fixes,
        GraphDiagnosticSeverity::Warning,
    )
}

fn diagnostic(
    flow: &InspectorFlow,
    subject: &str,
    code: &str,
    message: &str,
    fixes: Vec<&str>,
    severity: GraphDiagnosticSeverity,
) -> GraphDiagnostic {
    GraphDiagnostic {
        code: code.to_string(),
        severity,
        flow_id: flow.id.clone(),
        subject: subject.to_string(),
        message: message.to_string(),
        evidence: vec![GraphDiagnosticEvidence {
            key: "diagramKind".to_string(),
            value: flow.graph.kind.as_str().to_string(),
        }],
        supported_fixes: fixes.into_iter().map(str::to_string).collect(),
    }
}

#[cfg(test)]
mod tests {
    use crate::inspector::{
        DiagramKind, EntityStatus, FlowEdgeKind, InspectorFlowEdge, InspectorFlowNode,
        InspectorGraph,
    };

    use super::*;

    fn flow() -> InspectorFlow {
        InspectorFlow {
            id: "new:flow:checkout".to_string(),
            project_id: "project".to_string(),
            module_id: "module".to_string(),
            name: "Checkout".to_string(),
            summary: "Completes an order.".to_string(),
            status: EntityStatus::Ready,
            revision: 1,
            created_at: "1".to_string(),
            updated_at: "1".to_string(),
            graph: InspectorGraph {
                kind: DiagramKind::Workflow,
                nodes: vec![
                    InspectorFlowNode {
                        id: "new:node:start".to_string(),
                        label: "Receive order".to_string(),
                        kind: FlowNodeKind::Start,
                        description: None,
                        source_ref_ids: vec![],
                        position: 0,
                    },
                    InspectorFlowNode {
                        id: "new:node:end".to_string(),
                        label: "Confirm order".to_string(),
                        kind: FlowNodeKind::End,
                        description: None,
                        source_ref_ids: vec![],
                        position: 1,
                    },
                ],
                edges: vec![InspectorFlowEdge {
                    id: "new:edge:complete".to_string(),
                    from: "new:node:start".to_string(),
                    to: "new:node:end".to_string(),
                    label: Some("accepted".to_string()),
                    kind: FlowEdgeKind::Control,
                    mechanism: Some("direct call".to_string()),
                    source_ref_ids: vec![],
                }],
            },
            explanation: None,
            source_refs: vec![],
        }
    }

    #[test]
    fn accepts_a_connected_start_to_end_flow_with_evidence_warning() {
        let report = validate_renderable_flow(&flow()).expect("renderable flow");
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "graph.edge.evidence-unverified"));
    }

    #[test]
    fn returns_structured_diagnostics_for_invalid_graphs() {
        let mut invalid = flow();
        invalid.graph.nodes[1].kind = FlowNodeKind::Action;
        let Err(AppError::RenderError(report)) = validate_renderable_flow(&invalid) else {
            panic!("expected structured render error");
        };
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "graph.end.missing"
                && diagnostic.subject == "graph.nodes"
                && !diagnostic.supported_fixes.is_empty()));
    }

    #[test]
    fn decision_branch_with_whitespace_only_label_is_rejected() {
        let mut decision_flow = flow();
        decision_flow.graph.nodes[0].kind = FlowNodeKind::Decision;
        decision_flow.graph.edges[0].label = Some("   ".to_string());
        decision_flow.graph.edges.push(InspectorFlowEdge {
            id: "new:edge:decline".to_string(),
            from: "new:node:start".to_string(),
            to: "new:node:end".to_string(),
            label: Some("declined".to_string()),
            kind: FlowEdgeKind::Control,
            mechanism: None,
            source_ref_ids: vec![],
        });
        let Err(AppError::RenderError(report)) = validate_renderable_flow(&decision_flow) else {
            panic!("whitespace-only branch label must fail");
        };
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "graph.decision.branches-invalid"));
    }

    #[test]
    fn display_summary_prefers_the_first_error_diagnostic() {
        let mut invalid = flow();
        for index in 0..11 {
            invalid.graph.nodes.push(InspectorFlowNode {
                id: format!("new:node:extra:{index}"),
                label: format!("Extra step {index}"),
                kind: FlowNodeKind::Action,
                description: None,
                source_ref_ids: vec![],
                position: index + 2,
            });
        }
        invalid.graph.edges.clear();
        let Err(AppError::RenderError(report)) = validate_renderable_flow(&invalid) else {
            panic!("expected structured render error");
        };
        // The too-dense warning is recorded before the missing-edges error.
        assert_eq!(
            report.diagnostics[0].severity,
            GraphDiagnosticSeverity::Warning
        );
        let summary = report.to_string();
        assert!(summary.starts_with("Business flow steps must be connected."));
    }
}
