use std::collections::{HashMap, HashSet};

use crate::{
    error::AppError,
    inspector::{
        InspectorExplanation, InspectorFlow, InspectorModule, InspectorSnapshot, InspectorSourceRef,
    },
};

use super::{InspectorCommandScope, InspectorScopeKind, StructuralSummary};

pub(super) fn analyze_structure(
    current: &InspectorSnapshot,
    proposal: &InspectorSnapshot,
) -> StructuralSummary {
    let current_modules = current
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module))
        .collect::<HashMap<_, _>>();
    let proposed_modules = proposal
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module))
        .collect::<HashMap<_, _>>();
    let current_flows = flows_by_id(current);
    let proposed_flows = flows_by_id(proposal);
    let mut summary = StructuralSummary::default();

    for module in &proposal.modules {
        match current_modules.get(module.id.as_str()) {
            None => summary.added_modules.push(module.name.clone()),
            Some(previous) => {
                if previous.name != module.name {
                    summary.renamed_modules.push(module.name.clone());
                }
                if previous.parent_id != module.parent_id {
                    summary.moved_modules.push(module.name.clone());
                }
            }
        }
    }
    for module in &current.modules {
        if !proposed_modules.contains_key(module.id.as_str()) {
            summary.removed_modules.push(module.name.clone());
        }
    }
    for flow in proposal.modules.iter().flat_map(|module| &module.flows) {
        match current_flows.get(flow.id.as_str()) {
            None => summary.added_flows.push(flow.name.clone()),
            Some(previous) => {
                if previous.name != flow.name {
                    summary.renamed_flows.push(flow.name.clone());
                }
                if previous.module_id != flow.module_id {
                    summary.moved_flows.push(flow.name.clone());
                }
            }
        }
    }
    for flow in current.modules.iter().flat_map(|module| &module.flows) {
        if !proposed_flows.contains_key(flow.id.as_str()) {
            summary.removed_flows.push(flow.name.clone());
        }
    }
    sort_summary(&mut summary);
    summary
}

pub(super) fn validate_stable_ids(
    current: &InspectorSnapshot,
    proposal: &InspectorSnapshot,
) -> Result<(), AppError> {
    let module_ids = current
        .modules
        .iter()
        .map(|module| module.id.as_str())
        .collect::<HashSet<_>>();
    let modules_by_slug = current
        .modules
        .iter()
        .map(|module| (module.slug.as_str(), module.id.as_str()))
        .collect::<HashMap<_, _>>();
    for module in &proposal.modules {
        if !module_ids.contains(module.id.as_str())
            && modules_by_slug
                .get(module.slug.as_str())
                .is_some_and(|id| *id != module.id)
        {
            return Err(AppError::Validation(format!(
                "inspector-sync must preserve the module ID for slug {}",
                module.slug
            )));
        }
    }

    let flow_ids = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .map(|flow| flow.id.as_str())
        .collect::<HashSet<_>>();
    let flows_by_identity = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .map(|flow| {
            (
                (flow.module_id.as_str(), flow.name.as_str()),
                flow.id.as_str(),
            )
        })
        .collect::<HashMap<_, _>>();
    for flow in proposal.modules.iter().flat_map(|module| &module.flows) {
        if !flow_ids.contains(flow.id.as_str())
            && flows_by_identity
                .get(&(flow.module_id.as_str(), flow.name.as_str()))
                .is_some_and(|id| *id != flow.id)
        {
            return Err(AppError::Validation(format!(
                "inspector-sync must preserve the flow ID for {}",
                flow.name
            )));
        }
    }

    let current_sources = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.source_refs)
        .map(|source| {
            (
                (
                    source.flow_id.as_str(),
                    source.path.as_str(),
                    source.start_line,
                    source.end_line,
                    source.symbol.as_deref(),
                ),
                source.id.as_str(),
            )
        })
        .collect::<HashMap<_, _>>();
    for source in proposal
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .flat_map(|flow| &flow.source_refs)
    {
        let identity = (
            source.flow_id.as_str(),
            source.path.as_str(),
            source.start_line,
            source.end_line,
            source.symbol.as_deref(),
        );
        if current_sources
            .get(&identity)
            .is_some_and(|id| *id != source.id)
        {
            return Err(AppError::Validation(format!(
                "inspector-sync must preserve the source reference ID for {}",
                source.path
            )));
        }
    }
    Ok(())
}

pub(super) fn ensure_scope_unchanged(
    current: &InspectorSnapshot,
    proposal: &InspectorSnapshot,
    scope: &InspectorCommandScope,
) -> Result<(), AppError> {
    match scope.kind {
        InspectorScopeKind::Project => Ok(()),
        InspectorScopeKind::Module => ensure_module_scope_unchanged(current, proposal, scope),
        InspectorScopeKind::Flow => ensure_flow_scope_unchanged(current, proposal, scope),
    }
}

fn ensure_module_scope_unchanged(
    current: &InspectorSnapshot,
    proposal: &InspectorSnapshot,
    scope: &InspectorCommandScope,
) -> Result<(), AppError> {
    let target_id = scope
        .entity_id
        .as_deref()
        .ok_or_else(|| AppError::Validation("module scope requires entityId".to_string()))?;
    let allowed = module_subtree_ids(current, target_id);
    let proposed_by_id = proposal
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module))
        .collect::<HashMap<_, _>>();

    for module in &current.modules {
        if allowed.contains(module.id.as_str()) {
            continue;
        }
        let proposed = proposed_by_id.get(module.id.as_str()).ok_or_else(|| {
            AppError::Validation(format!(
                "module {} changed outside the selected scope",
                module.id
            ))
        })?;
        if !module_full_eq(module, proposed) {
            return Err(AppError::Validation(format!(
                "module {} changed outside the selected scope",
                module.id
            )));
        }
    }

    for module in &proposal.modules {
        if current.modules.iter().any(|item| item.id == module.id) {
            continue;
        }
        if !descends_from_proposed(proposal, module, target_id) {
            return Err(AppError::Validation(format!(
                "new module {} is outside the selected scope",
                module.id
            )));
        }
    }
    Ok(())
}

fn ensure_flow_scope_unchanged(
    current: &InspectorSnapshot,
    proposal: &InspectorSnapshot,
    scope: &InspectorCommandScope,
) -> Result<(), AppError> {
    let target_id = scope
        .entity_id
        .as_deref()
        .ok_or_else(|| AppError::Validation("flow scope requires entityId".to_string()))?;
    let target = current
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .find(|flow| flow.id == target_id)
        .ok_or_else(|| AppError::NotFound(format!("flow {target_id}")))?;
    let current_module_ids = current
        .modules
        .iter()
        .map(|module| module.id.as_str())
        .collect::<HashSet<_>>();
    let proposed_by_id = proposal
        .modules
        .iter()
        .map(|module| (module.id.as_str(), module))
        .collect::<HashMap<_, _>>();

    for module in &proposal.modules {
        if !current_module_ids.contains(module.id.as_str()) {
            return Err(AppError::Validation(
                "flow scope cannot create modules".to_string(),
            ));
        }
    }
    for module in &current.modules {
        let proposed = proposed_by_id.get(module.id.as_str()).ok_or_else(|| {
            AppError::Validation(format!(
                "module {} changed outside the selected scope",
                module.id
            ))
        })?;
        if module.id != target.module_id && !module_full_eq(module, proposed) {
            return Err(AppError::Validation(format!(
                "module {} changed outside the selected scope",
                module.id
            )));
        }
        if module.id == target.module_id && !module_own_eq(module, proposed) {
            return Err(AppError::Validation(
                "flow scope cannot change its parent module metadata".to_string(),
            ));
        }
        for flow in &module.flows {
            if flow.id == target_id {
                continue;
            }
            let candidate = proposed.flows.iter().find(|item| item.id == flow.id);
            if candidate.is_none_or(|item| !flow_content_eq(flow, item)) {
                return Err(AppError::Validation(format!(
                    "flow {} changed outside the selected scope",
                    flow.id
                )));
            }
        }
    }
    for module in &proposal.modules {
        for flow in &module.flows {
            let is_new = !current
                .modules
                .iter()
                .flat_map(|item| &item.flows)
                .any(|item| item.id == flow.id);
            if is_new && module.id != target.module_id {
                return Err(AppError::Validation(format!(
                    "new flow {} is outside the selected scope",
                    flow.id
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn module_own_eq(left: &InspectorModule, right: &InspectorModule) -> bool {
    left.project_id == right.project_id
        && left.parent_id == right.parent_id
        && left.name == right.name
        && left.slug == right.slug
        && left.summary == right.summary
        && left.status == right.status
}

pub(super) fn flow_content_eq(left: &InspectorFlow, right: &InspectorFlow) -> bool {
    left.project_id == right.project_id
        && left.module_id == right.module_id
        && left.name == right.name
        && left.summary == right.summary
        && left.status == right.status
        && left.graph == right.graph
        && explanations_eq(left.explanation.as_ref(), right.explanation.as_ref())
        && sources_eq(&left.source_refs, &right.source_refs)
}

pub(super) fn source_content_eq(left: &InspectorSourceRef, right: &InspectorSourceRef) -> bool {
    left.project_id == right.project_id
        && left.flow_id == right.flow_id
        && left.path == right.path
        && left.start_line == right.start_line
        && left.end_line == right.end_line
        && left.start_column == right.start_column
        && left.end_column == right.end_column
        && left.symbol == right.symbol
        && left.label == right.label
        && left.content_hash == right.content_hash
        && left.status == right.status
}

pub(super) fn explanation_content_eq(
    left: &InspectorExplanation,
    right: &InspectorExplanation,
) -> bool {
    left.project_id == right.project_id
        && left.flow_id == right.flow_id
        && left.headline == right.headline
        && left.body == right.body
        && left.decisions == right.decisions
        && left.confidence == right.confidence
        && left.provenance == right.provenance
}

fn module_full_eq(left: &InspectorModule, right: &InspectorModule) -> bool {
    if !module_own_eq(left, right) || left.flows.len() != right.flows.len() {
        return false;
    }
    left.flows.iter().all(|flow| {
        right
            .flows
            .iter()
            .find(|candidate| candidate.id == flow.id)
            .is_some_and(|candidate| flow_content_eq(flow, candidate))
    })
}

fn explanations_eq(
    left: Option<&InspectorExplanation>,
    right: Option<&InspectorExplanation>,
) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => explanation_content_eq(left, right),
        (None, None) => true,
        _ => false,
    }
}

fn sources_eq(left: &[InspectorSourceRef], right: &[InspectorSourceRef]) -> bool {
    left.len() == right.len()
        && left.iter().all(|source| {
            right
                .iter()
                .find(|candidate| candidate.id == source.id)
                .is_some_and(|candidate| source_content_eq(source, candidate))
        })
}

fn module_subtree_ids<'a>(snapshot: &'a InspectorSnapshot, target_id: &'a str) -> HashSet<&'a str> {
    let mut result = HashSet::from([target_id]);
    loop {
        let before = result.len();
        for module in &snapshot.modules {
            if module
                .parent_id
                .as_deref()
                .is_some_and(|parent| result.contains(parent))
            {
                result.insert(module.id.as_str());
            }
        }
        if result.len() == before {
            return result;
        }
    }
}

fn descends_from_proposed(
    proposal: &InspectorSnapshot,
    module: &InspectorModule,
    target_id: &str,
) -> bool {
    let by_id = proposal
        .modules
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let mut parent = module.parent_id.as_deref();
    let mut visited = HashSet::new();
    while let Some(parent_id) = parent {
        if parent_id == target_id {
            return true;
        }
        if !visited.insert(parent_id) {
            return false;
        }
        parent = by_id
            .get(parent_id)
            .and_then(|item| item.parent_id.as_deref());
    }
    false
}

fn flows_by_id(snapshot: &InspectorSnapshot) -> HashMap<&str, &InspectorFlow> {
    snapshot
        .modules
        .iter()
        .flat_map(|module| &module.flows)
        .map(|flow| (flow.id.as_str(), flow))
        .collect()
}

fn sort_summary(summary: &mut StructuralSummary) {
    summary.added_modules.sort();
    summary.removed_modules.sort();
    summary.renamed_modules.sort();
    summary.moved_modules.sort();
    summary.added_flows.sort();
    summary.removed_flows.sort();
    summary.renamed_flows.sort();
    summary.moved_flows.sort();
}
