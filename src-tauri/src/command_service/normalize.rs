use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::{
    error::AppError,
    inspector::{
        EntityStatus, InspectorExplanation, InspectorFlow, InspectorModule, InspectorSnapshot,
        InspectorSourceRef, SourceRefStatus,
    },
};

use super::{
    diff::{explanation_content_eq, flow_content_eq, module_own_eq, source_content_eq},
    RunInspectorCommandInput,
};

pub(super) fn normalize_proposal(
    current: &InspectorSnapshot,
    mut proposal: InspectorSnapshot,
    input: &RunInspectorCommandInput,
) -> Result<InspectorSnapshot, AppError> {
    reject_archived_entities(&proposal)?;
    let current_module_ids = current
        .modules
        .iter()
        .map(|module| module.id.clone())
        .collect::<HashSet<_>>();
    let current_flow_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .map(|flow| flow.id.clone())
        .collect::<HashSet<_>>();
    let current_node_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.graph.nodes)
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let current_edge_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.graph.edges)
        .map(|edge| edge.id.clone())
        .collect::<HashSet<_>>();
    let current_source_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.source_refs)
        .map(|source| source.id.clone())
        .collect::<HashSet<_>>();
    let current_explanation_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .filter_map(|flow| flow.explanation.as_ref())
        .map(|explanation| explanation.id.clone())
        .collect::<HashSet<_>>();

    let mut module_ids = assign_ids(
        proposal.modules.iter().map(|module| module.id.as_str()),
        &current_module_ids,
        "module",
    )?;
    let mut flow_ids = assign_ids(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .map(|flow| flow.id.as_str()),
        &current_flow_ids,
        "flow",
    )?;
    let mut node_ids = assign_ids(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.graph.nodes)
            .map(|node| node.id.as_str()),
        &current_node_ids,
        "flow node",
    )?;
    let mut edge_ids = assign_ids(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.graph.edges)
            .map(|edge| edge.id.as_str()),
        &current_edge_ids,
        "flow edge",
    )?;
    let mut source_ids = assign_ids(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.source_refs)
            .map(|source| source.id.as_str()),
        &current_source_ids,
        "source reference",
    )?;
    let mut explanation_ids = assign_ids(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .filter_map(|flow| flow.explanation.as_ref())
            .map(|explanation| explanation.id.as_str()),
        &current_explanation_ids,
        "explanation",
    )?;

    for module in &mut proposal.modules {
        module.id = take_assigned(&mut module_ids, &module.id, &current_module_ids, "module")?;
        module.project_id = input.project_id.clone();
        if let Some(parent_id) = &module.parent_id {
            module.parent_id = Some(resolve_reference(
                parent_id,
                &module_ids,
                &current_module_ids,
                "module parent",
            )?);
        }
        for flow in &mut module.flows {
            flow.id = take_assigned(&mut flow_ids, &flow.id, &current_flow_ids, "flow")?;
            flow.project_id = input.project_id.clone();
            flow.module_id = module.id.clone();
            for source in &mut flow.source_refs {
                source.id = take_assigned(
                    &mut source_ids,
                    &source.id,
                    &current_source_ids,
                    "source reference",
                )?;
                source.project_id = input.project_id.clone();
                source.flow_id = flow.id.clone();
            }
            for node in &mut flow.graph.nodes {
                node.id = take_assigned(&mut node_ids, &node.id, &current_node_ids, "flow node")?;
                node.source_ref_ids = node
                    .source_ref_ids
                    .iter()
                    .map(|id| {
                        resolve_reference(
                            id,
                            &source_ids,
                            &current_source_ids,
                            "node source reference",
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            for edge in &mut flow.graph.edges {
                edge.id = take_assigned(&mut edge_ids, &edge.id, &current_edge_ids, "flow edge")?;
                edge.from = resolve_reference(
                    &edge.from,
                    &node_ids,
                    &current_node_ids,
                    "edge source node",
                )?;
                edge.to =
                    resolve_reference(&edge.to, &node_ids, &current_node_ids, "edge target node")?;
            }
            if let Some(explanation) = &mut flow.explanation {
                explanation.id = take_assigned(
                    &mut explanation_ids,
                    &explanation.id,
                    &current_explanation_ids,
                    "explanation",
                )?;
                explanation.project_id = input.project_id.clone();
                explanation.flow_id = flow.id.clone();
                explanation.provenance.evidence_ids = explanation
                    .provenance
                    .evidence_ids
                    .iter()
                    .map(|id| {
                        resolve_reference(
                            id,
                            &source_ids,
                            &current_source_ids,
                            "provenance evidence",
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
            }
            validate_flow_references(flow)?;
        }
    }
    ensure_unique_ids(&proposal)?;
    apply_server_metadata(current, &mut proposal, input)?;
    proposal.project_revision = current.project_revision;
    Ok(proposal)
}

fn assign_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    current_ids: &HashSet<String>,
    kind: &str,
) -> Result<HashMap<String, String>, AppError> {
    let mut assigned = HashMap::new();
    for id in ids {
        if current_ids.contains(id) {
            continue;
        }
        validate_temporary_id(id, kind)?;
        assigned
            .entry(id.to_string())
            .or_insert_with(|| Uuid::now_v7().to_string());
    }
    Ok(assigned)
}

fn take_assigned(
    assigned: &mut HashMap<String, String>,
    id: &str,
    current_ids: &HashSet<String>,
    kind: &str,
) -> Result<String, AppError> {
    if current_ids.contains(id) {
        return Ok(id.to_string());
    }
    assigned
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::Validation(format!("unknown {kind} ID {id}")))
}

fn resolve_reference(
    id: &str,
    assigned: &HashMap<String, String>,
    current_ids: &HashSet<String>,
    kind: &str,
) -> Result<String, AppError> {
    if current_ids.contains(id) {
        return Ok(id.to_string());
    }
    assigned
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::Validation(format!("unknown {kind} ID {id}")))
}

fn validate_temporary_id(id: &str, kind: &str) -> Result<(), AppError> {
    if !id.starts_with("new:") || id.len() <= 4 || id.len() > 160 {
        return Err(AppError::Validation(format!(
            "new {kind} IDs must use a non-empty new: prefix"
        )));
    }
    Ok(())
}

fn reject_archived_entities(proposal: &InspectorSnapshot) -> Result<(), AppError> {
    for module in &proposal.modules {
        if module.status == EntityStatus::Archived {
            return Err(AppError::Validation(
                "active proposals cannot contain archived modules".to_string(),
            ));
        }
        for flow in &module.flows {
            if flow.status == EntityStatus::Archived {
                return Err(AppError::Validation(
                    "active proposals cannot contain archived flows".to_string(),
                ));
            }
            if flow
                .source_refs
                .iter()
                .any(|source| source.status == SourceRefStatus::Archived)
            {
                return Err(AppError::Validation(
                    "active proposals cannot contain archived source references".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_flow_references(flow: &InspectorFlow) -> Result<(), AppError> {
    let source_ids = flow
        .source_refs
        .iter()
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    for node in &flow.graph.nodes {
        if node
            .source_ref_ids
            .iter()
            .any(|id| !source_ids.contains(id.as_str()))
        {
            return Err(AppError::Validation(format!(
                "flow node {} references evidence outside its flow",
                node.id
            )));
        }
    }
    if let Some(explanation) = &flow.explanation {
        if explanation
            .provenance
            .evidence_ids
            .iter()
            .any(|id| !source_ids.contains(id.as_str()))
        {
            return Err(AppError::Validation(format!(
                "explanation {} references evidence outside its flow",
                explanation.id
            )));
        }
    }
    Ok(())
}

fn ensure_unique_ids(proposal: &InspectorSnapshot) -> Result<(), AppError> {
    ensure_unique(
        proposal.modules.iter().map(|module| module.id.as_str()),
        "module",
    )?;
    ensure_unique(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .map(|flow| flow.id.as_str()),
        "flow",
    )?;
    ensure_unique(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.graph.nodes)
            .map(|node| node.id.as_str()),
        "flow node",
    )?;
    ensure_unique(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.graph.edges)
            .map(|edge| edge.id.as_str()),
        "flow edge",
    )?;
    ensure_unique(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .flat_map(|flow| &flow.source_refs)
            .map(|source| source.id.as_str()),
        "source reference",
    )?;
    ensure_unique(
        proposal
            .modules
            .iter()
            .flat_map(|module| &module.flows)
            .filter_map(|flow| flow.explanation.as_ref())
            .map(|explanation| explanation.id.as_str()),
        "explanation",
    )
}

fn ensure_unique<'a>(ids: impl Iterator<Item = &'a str>, kind: &str) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(AppError::Validation(format!("duplicate {kind} ID {id}")));
        }
    }
    Ok(())
}

fn apply_server_metadata(
    current: &InspectorSnapshot,
    proposal: &mut InspectorSnapshot,
    input: &RunInspectorCommandInput,
) -> Result<(), AppError> {
    let now = timestamp();
    let current_modules = current
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module))
        .collect::<HashMap<_, _>>();
    let current_flows = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .map(|flow| (flow.id.as_str(), flow))
        .collect::<HashMap<_, _>>();
    let current_sources = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.source_refs)
        .map(|source| (source.id.as_str(), source))
        .collect::<HashMap<_, _>>();
    let current_explanations = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .filter_map(|flow| flow.explanation.as_ref())
        .map(|explanation| (explanation.id.as_str(), explanation))
        .collect::<HashMap<_, _>>();

    for module in &mut proposal.modules {
        apply_module_metadata(
            module,
            current_modules.get(module.id.as_str()).copied(),
            &now,
        )?;
        for flow in &mut module.flows {
            for source in &mut flow.source_refs {
                apply_source_metadata(
                    source,
                    current_sources.get(source.id.as_str()).copied(),
                    &now,
                )?;
            }
            if let Some(explanation) = &mut flow.explanation {
                apply_explanation_metadata(
                    explanation,
                    current_explanations.get(explanation.id.as_str()).copied(),
                    input,
                    &now,
                )?;
            }
            apply_flow_metadata(flow, current_flows.get(flow.id.as_str()).copied(), &now)?;
        }
    }
    Ok(())
}

fn apply_module_metadata(
    module: &mut InspectorModule,
    current: Option<&InspectorModule>,
    now: &str,
) -> Result<(), AppError> {
    if let Some(current) = current {
        let changed = !module_own_eq(current, module);
        module.created_at = current.created_at.clone();
        module.revision = next_revision(current.revision, changed)?;
        module.updated_at = if changed {
            now.to_string()
        } else {
            current.updated_at.clone()
        };
    } else {
        module.revision = 1;
        module.created_at = now.to_string();
        module.updated_at = now.to_string();
    }
    Ok(())
}

fn apply_flow_metadata(
    flow: &mut InspectorFlow,
    current: Option<&InspectorFlow>,
    now: &str,
) -> Result<(), AppError> {
    if let Some(current) = current {
        let changed = !flow_content_eq(current, flow);
        flow.created_at = current.created_at.clone();
        flow.revision = next_revision(current.revision, changed)?;
        flow.updated_at = if changed {
            now.to_string()
        } else {
            current.updated_at.clone()
        };
    } else {
        flow.revision = 1;
        flow.created_at = now.to_string();
        flow.updated_at = now.to_string();
    }
    Ok(())
}

fn apply_source_metadata(
    source: &mut InspectorSourceRef,
    current: Option<&InspectorSourceRef>,
    now: &str,
) -> Result<(), AppError> {
    if let Some(current) = current {
        let changed = !source_content_eq(current, source);
        source.created_at = current.created_at.clone();
        source.revision = next_revision(current.revision, changed)?;
        source.updated_at = if changed {
            now.to_string()
        } else {
            current.updated_at.clone()
        };
    } else {
        source.revision = 1;
        source.created_at = now.to_string();
        source.updated_at = now.to_string();
    }
    Ok(())
}

fn apply_explanation_metadata(
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
        explanation.revision = next_revision(current.revision, changed)?;
        explanation.updated_at = if changed {
            now.to_string()
        } else {
            current.updated_at.clone()
        };
    } else {
        explanation.revision = 1;
        explanation.created_at = now.to_string();
        explanation.updated_at = now.to_string();
    }
    Ok(())
}

fn next_revision(current: u64, changed: bool) -> Result<u64, AppError> {
    if !changed {
        return Ok(current);
    }
    current
        .checked_add(1)
        .ok_or_else(|| AppError::Database("entity revision overflow".to_string()))
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
