use std::{collections::HashSet, path::Path};

use rusqlite::{params, OptionalExtension, Transaction};
use uuid::Uuid;

use crate::{
    error::AppError,
    inspector::{
        EntityStatus, FlowNodeKind, InspectorExplanation, InspectorFlow, InspectorFlowEdge,
        InspectorFlowNode, InspectorGraph, InspectorModule, InspectorSnapshot, InspectorSourceRef,
        Provenance, SourceRefStatus,
    },
};

use super::{
    database_error, record_mutation, revision_from_sql, revision_to_sql, timestamp,
    InspectorDatabase,
};

impl InspectorDatabase {
    pub fn load_inspector_snapshot(&self, project_id: &str) -> Result<InspectorSnapshot, AppError> {
        let connection = self.connect()?;
        let project_revision = connection
            .query_row(
                "SELECT revision FROM projects WHERE id = ?1 AND archived_at IS NULL",
                [project_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;
        let mut statement = connection
            .prepare(
                "SELECT id, parent_id, name, slug, summary, status, revision, created_at, updated_at
                 FROM modules WHERE project_id = ?1 AND archived_at IS NULL
                 ORDER BY parent_id IS NOT NULL, name, id",
            )
            .map_err(database_error)?;
        let module_rows = statement
            .query_map([project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })
            .map_err(database_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(database_error)?;

        let mut modules = Vec::with_capacity(module_rows.len());
        for row in module_rows {
            let module_id = row.0;
            modules.push(InspectorModule {
                flows: load_flows(&connection, project_id, &module_id)?,
                id: module_id,
                project_id: project_id.to_string(),
                parent_id: row.1,
                name: row.2,
                slug: row.3,
                summary: row.4,
                status: parse_entity_status(&row.5)?,
                revision: revision_from_sql(row.6)?,
                created_at: row.7,
                updated_at: row.8,
            });
        }

        Ok(InspectorSnapshot {
            project_id: project_id.to_string(),
            project_revision: revision_from_sql(project_revision)?,
            modules,
        })
    }

    pub fn commit_inspector_snapshot(
        &self,
        snapshot: &InspectorSnapshot,
        expected_project_revision: u64,
        actor: &str,
        source: &str,
        request_id: &str,
    ) -> Result<InspectorSnapshot, AppError> {
        validate_snapshot(snapshot)?;
        let mut connection = self.connect()?;
        if let Some(result) = connection
            .query_row(
                "SELECT result_json FROM mutation_requests WHERE request_id = ?1",
                [request_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(database_error)?
        {
            return serde_json::from_str(&result).map_err(database_error);
        }

        let transaction = connection.transaction().map_err(database_error)?;
        let actual_revision = transaction
            .query_row(
                "SELECT revision FROM projects WHERE id = ?1 AND archived_at IS NULL",
                [&snapshot.project_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(database_error)?
            .ok_or_else(|| AppError::NotFound(format!("project {}", snapshot.project_id)))?;
        let actual_revision = revision_from_sql(actual_revision)?;
        if actual_revision != expected_project_revision {
            return Err(AppError::RevisionConflict {
                entity_id: snapshot.project_id.clone(),
                expected: expected_project_revision,
                actual: actual_revision,
            });
        }
        let next_project_revision = actual_revision
            .checked_add(1)
            .ok_or_else(|| AppError::Database("project revision overflow".to_string()))?;
        let committed_at = timestamp();

        archive_missing_entities(&transaction, snapshot, &committed_at)?;
        for module in ordered_modules(snapshot)? {
            upsert_module(&transaction, module)?;
        }
        for module in &snapshot.modules {
            for flow in &module.flows {
                upsert_flow(&transaction, flow)?;
            }
        }
        transaction
            .execute(
                "UPDATE projects SET revision = ?1, updated_at = ?2 WHERE id = ?3",
                params![
                    revision_to_sql(next_project_revision)?,
                    committed_at,
                    snapshot.project_id
                ],
            )
            .map_err(database_error)?;
        record_mutation(
            &transaction,
            Some(&snapshot.project_id),
            "inspector_snapshot",
            &snapshot.project_id,
            "commit",
            actor,
            source,
            next_project_revision,
            Some(request_id),
            &committed_at,
        )?;

        let committed = canonical_snapshot(snapshot, next_project_revision);
        let result_json = serde_json::to_string(&committed).map_err(database_error)?;
        transaction
            .execute(
                "INSERT INTO mutation_requests (request_id, actor, operation, result_json, created_at)
                 VALUES (?1, ?2, 'inspector.snapshot.commit', ?3, ?4)",
                params![request_id, actor, result_json, committed_at],
            )
            .map_err(database_error)?;
        transaction.commit().map_err(database_error)?;
        Ok(committed)
    }
}

fn canonical_snapshot(snapshot: &InspectorSnapshot, project_revision: u64) -> InspectorSnapshot {
    let mut committed = snapshot.clone();
    committed.project_revision = project_revision;
    committed.modules.sort_by(|left, right| {
        left.parent_id
            .is_some()
            .cmp(&right.parent_id.is_some())
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    for module in &mut committed.modules {
        module.flows.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        for flow in &mut module.flows {
            flow.graph.nodes.sort_by(|left, right| {
                left.position
                    .cmp(&right.position)
                    .then_with(|| left.id.cmp(&right.id))
            });
            flow.graph
                .edges
                .sort_by(|left, right| left.id.cmp(&right.id));
            flow.source_refs.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then_with(|| left.start_line.cmp(&right.start_line))
                    .then_with(|| left.id.cmp(&right.id))
            });
        }
    }
    committed
}

fn load_flows(
    connection: &rusqlite::Connection,
    project_id: &str,
    module_id: &str,
) -> Result<Vec<InspectorFlow>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, summary, status, revision, created_at, updated_at
             FROM flows WHERE module_id = ?1 AND archived_at IS NULL ORDER BY name, id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([module_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| {
            let flow_id = row.0;
            Ok(InspectorFlow {
                graph: load_graph(connection, &flow_id)?,
                explanation: load_explanation(connection, &flow_id)?,
                source_refs: load_source_refs(connection, project_id, &flow_id)?,
                id: flow_id,
                project_id: project_id.to_string(),
                module_id: module_id.to_string(),
                name: row.1,
                summary: row.2,
                status: parse_entity_status(&row.3)?,
                revision: revision_from_sql(row.4)?,
                created_at: row.5,
                updated_at: row.6,
            })
        })
        .collect()
}

fn load_graph(
    connection: &rusqlite::Connection,
    flow_id: &str,
) -> Result<InspectorGraph, AppError> {
    let mut node_statement = connection
        .prepare(
            "SELECT id, label, kind, description, position, source_ref_ids_json
             FROM flow_steps WHERE flow_id = ?1 ORDER BY position, id",
        )
        .map_err(database_error)?;
    let nodes = node_statement
        .query_map([flow_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?
        .into_iter()
        .map(|row| {
            Ok(InspectorFlowNode {
                id: row.0,
                label: row.1,
                kind: FlowNodeKind::parse(&row.2)
                    .ok_or_else(|| AppError::Database(format!("unknown node kind {}", row.2)))?,
                description: row.3,
                position: row.4,
                source_ref_ids: serde_json::from_str(&row.5).map_err(database_error)?,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    let mut edge_statement = connection
        .prepare(
            "SELECT id, from_step_id, to_step_id, label FROM flow_edges
             WHERE flow_id = ?1 ORDER BY id",
        )
        .map_err(database_error)?;
    let edges = edge_statement
        .query_map([flow_id], |row| {
            Ok(InspectorFlowEdge {
                id: row.get(0)?,
                from: row.get(1)?,
                to: row.get(2)?,
                label: row.get(3)?,
            })
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    Ok(InspectorGraph { nodes, edges })
}

fn load_source_refs(
    connection: &rusqlite::Connection,
    project_id: &str,
    flow_id: &str,
) -> Result<Vec<InspectorSourceRef>, AppError> {
    let mut statement = connection
        .prepare(
            "SELECT id, path, start_line, end_line, start_column, end_column, symbol, label,
                    content_hash, status, revision, created_at, updated_at
             FROM source_refs WHERE flow_id = ?1 AND archived_at IS NULL
             ORDER BY path, start_line, id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([flow_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
            ))
        })
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(InspectorSourceRef {
                id: row.0,
                project_id: project_id.to_string(),
                flow_id: flow_id.to_string(),
                path: row.1,
                start_line: usize_from_sql(row.2)?,
                end_line: usize_from_sql(row.3)?,
                start_column: row.4.map(usize_from_sql).transpose()?,
                end_column: row.5.map(usize_from_sql).transpose()?,
                symbol: row.6,
                label: row.7,
                content_hash: row.8,
                status: SourceRefStatus::parse(&row.9).ok_or_else(|| {
                    AppError::Database(format!("unknown source status {}", row.9))
                })?,
                revision: revision_from_sql(row.10)?,
                created_at: row.11,
                updated_at: row.12,
            })
        })
        .collect()
}

fn load_explanation(
    connection: &rusqlite::Connection,
    flow_id: &str,
) -> Result<Option<InspectorExplanation>, AppError> {
    let row = connection
        .query_row(
            "SELECT id, project_id, headline, body, decisions_json, confidence, provenance_json,
                    revision, created_at, updated_at
             FROM explanations WHERE flow_id = ?1 AND archived_at IS NULL ORDER BY revision DESC LIMIT 1",
            [flow_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )
        .optional()
        .map_err(database_error)?;
    row.map(|row| {
        Ok(InspectorExplanation {
            id: row.0,
            project_id: row.1,
            flow_id: flow_id.to_string(),
            headline: row.2,
            body: row.3,
            decisions: serde_json::from_str(&row.4).map_err(database_error)?,
            confidence: row.5,
            provenance: serde_json::from_str::<Provenance>(&row.6).map_err(database_error)?,
            revision: revision_from_sql(row.7)?,
            created_at: row.8,
            updated_at: row.9,
        })
    })
    .transpose()
}

fn upsert_module(transaction: &Transaction<'_>, module: &InspectorModule) -> Result<(), AppError> {
    transaction
        .execute(
            "INSERT INTO modules (
                id, project_id, parent_id, name, slug, summary, status, revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET parent_id = excluded.parent_id, name = excluded.name,
                slug = excluded.slug, summary = excluded.summary, status = excluded.status,
                revision = excluded.revision, updated_at = excluded.updated_at, archived_at = NULL",
            params![
                module.id,
                module.project_id,
                module.parent_id,
                module.name,
                module.slug,
                module.summary,
                module.status.as_str(),
                revision_to_sql(module.revision)?,
                module.created_at,
                module.updated_at,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn upsert_flow(transaction: &Transaction<'_>, flow: &InspectorFlow) -> Result<(), AppError> {
    transaction
        .execute(
            "INSERT INTO flows (
                id, project_id, module_id, name, summary, status, revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET module_id = excluded.module_id, name = excluded.name,
                summary = excluded.summary, status = excluded.status, revision = excluded.revision,
                updated_at = excluded.updated_at, archived_at = NULL",
            params![
                flow.id,
                flow.project_id,
                flow.module_id,
                flow.name,
                flow.summary,
                flow.status.as_str(),
                revision_to_sql(flow.revision)?,
                flow.created_at,
                flow.updated_at,
            ],
        )
        .map_err(database_error)?;
    transaction
        .execute("DELETE FROM flow_edges WHERE flow_id = ?1", [&flow.id])
        .map_err(database_error)?;
    transaction
        .execute("DELETE FROM flow_steps WHERE flow_id = ?1", [&flow.id])
        .map_err(database_error)?;
    for node in &flow.graph.nodes {
        transaction
            .execute(
                "INSERT INTO flow_steps (id, flow_id, label, kind, description, position, source_ref_ids_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    node.id,
                    flow.id,
                    node.label,
                    node.kind.as_str(),
                    node.description,
                    node.position,
                    serde_json::to_string(&node.source_ref_ids).map_err(database_error)?,
                ],
            )
            .map_err(database_error)?;
    }
    for edge in &flow.graph.edges {
        transaction
            .execute(
                "INSERT INTO flow_edges (id, flow_id, from_step_id, to_step_id, label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![edge.id, flow.id, edge.from, edge.to, edge.label],
            )
            .map_err(database_error)?;
    }
    upsert_source_refs(transaction, flow)?;
    upsert_explanation(transaction, flow)?;
    Ok(())
}

fn upsert_source_refs(transaction: &Transaction<'_>, flow: &InspectorFlow) -> Result<(), AppError> {
    let retained = flow
        .source_refs
        .iter()
        .map(|source| source.id.as_str())
        .collect::<HashSet<_>>();
    let mut statement = transaction
        .prepare("SELECT id FROM source_refs WHERE flow_id = ?1 AND archived_at IS NULL")
        .map_err(database_error)?;
    let existing = statement
        .query_map([&flow.id], |row| row.get::<_, String>(0))
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    drop(statement);
    for id in existing {
        if !retained.contains(id.as_str()) {
            transaction
                .execute(
                    "UPDATE source_refs SET status = 'archived', archived_at = ?1 WHERE id = ?2",
                    params![timestamp(), id],
                )
                .map_err(database_error)?;
        }
    }
    for source in &flow.source_refs {
        transaction
            .execute(
                "INSERT INTO source_refs (
                    id, project_id, flow_id, path, start_line, end_line, start_column, end_column,
                    symbol, label, content_hash, status, revision, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(id) DO UPDATE SET path = excluded.path, start_line = excluded.start_line,
                    end_line = excluded.end_line, start_column = excluded.start_column,
                    end_column = excluded.end_column, symbol = excluded.symbol, label = excluded.label,
                    content_hash = excluded.content_hash, status = excluded.status,
                    revision = excluded.revision, updated_at = excluded.updated_at, archived_at = NULL",
                params![
                    source.id,
                    source.project_id,
                    source.flow_id,
                    source.path,
                    i64_from_usize(source.start_line)?,
                    i64_from_usize(source.end_line)?,
                    source.start_column.map(i64_from_usize).transpose()?,
                    source.end_column.map(i64_from_usize).transpose()?,
                    source.symbol,
                    source.label,
                    source.content_hash,
                    source.status.as_str(),
                    revision_to_sql(source.revision)?,
                    source.created_at,
                    source.updated_at,
                ],
            )
            .map_err(database_error)?;
    }
    Ok(())
}

fn upsert_explanation(transaction: &Transaction<'_>, flow: &InspectorFlow) -> Result<(), AppError> {
    let Some(explanation) = &flow.explanation else {
        transaction
            .execute(
                "UPDATE explanations SET archived_at = ?1 WHERE flow_id = ?2 AND archived_at IS NULL",
                params![timestamp(), flow.id],
            )
            .map_err(database_error)?;
        return Ok(());
    };
    transaction
        .execute(
            "INSERT INTO explanations (
                id, project_id, flow_id, headline, body, decisions_json, confidence, provenance_json,
                revision, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET headline = excluded.headline, body = excluded.body,
                decisions_json = excluded.decisions_json, confidence = excluded.confidence,
                provenance_json = excluded.provenance_json, revision = excluded.revision,
                updated_at = excluded.updated_at, archived_at = NULL",
            params![
                explanation.id,
                explanation.project_id,
                explanation.flow_id,
                explanation.headline,
                explanation.body,
                serde_json::to_string(&explanation.decisions).map_err(database_error)?,
                explanation.confidence,
                serde_json::to_string(&explanation.provenance).map_err(database_error)?,
                revision_to_sql(explanation.revision)?,
                explanation.created_at,
                explanation.updated_at,
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn archive_missing_entities(
    transaction: &Transaction<'_>,
    snapshot: &InspectorSnapshot,
    archived_at: &str,
) -> Result<(), AppError> {
    let module_ids = snapshot
        .modules
        .iter()
        .map(|module| module.id.as_str())
        .collect::<HashSet<_>>();
    let flow_ids = snapshot
        .modules
        .iter()
        .flat_map(|module| module.flows.iter().map(|flow| flow.id.as_str()))
        .collect::<HashSet<_>>();
    let mut module_statement = transaction
        .prepare("SELECT id FROM modules WHERE project_id = ?1 AND archived_at IS NULL")
        .map_err(database_error)?;
    let existing_modules = module_statement
        .query_map([&snapshot.project_id], |row| row.get::<_, String>(0))
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    drop(module_statement);
    for id in existing_modules {
        if !module_ids.contains(id.as_str()) {
            transaction
                .execute(
                    "UPDATE modules SET status = 'archived', archived_at = ?1 WHERE id = ?2",
                    params![archived_at, id],
                )
                .map_err(database_error)?;
        }
    }
    let mut flow_statement = transaction
        .prepare("SELECT id FROM flows WHERE project_id = ?1 AND archived_at IS NULL")
        .map_err(database_error)?;
    let existing_flows = flow_statement
        .query_map([&snapshot.project_id], |row| row.get::<_, String>(0))
        .map_err(database_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(database_error)?;
    drop(flow_statement);
    for id in existing_flows {
        if !flow_ids.contains(id.as_str()) {
            transaction
                .execute(
                    "UPDATE flows SET status = 'archived', archived_at = ?1 WHERE id = ?2",
                    params![archived_at, id],
                )
                .map_err(database_error)?;
        }
    }
    Ok(())
}

fn validate_snapshot(snapshot: &InspectorSnapshot) -> Result<(), AppError> {
    validate_uuid(&snapshot.project_id)?;
    if snapshot.modules.len() > 1_000 {
        return Err(AppError::Validation("module limit exceeded".to_string()));
    }
    let module_ids = snapshot
        .modules
        .iter()
        .map(|module| module.id.as_str())
        .collect::<HashSet<_>>();
    if module_ids.len() != snapshot.modules.len() {
        return Err(AppError::Validation("duplicate module ids".to_string()));
    }
    let module_slugs = snapshot
        .modules
        .iter()
        .map(|module| module.slug.as_str())
        .collect::<HashSet<_>>();
    if module_slugs.len() != snapshot.modules.len() {
        return Err(AppError::Validation("duplicate module slugs".to_string()));
    }
    for module in &snapshot.modules {
        validate_uuid(&module.id)?;
        if module.project_id != snapshot.project_id || module.name.trim().is_empty() {
            return Err(AppError::Validation(format!(
                "invalid module {}",
                module.id
            )));
        }
        if let Some(parent_id) = &module.parent_id {
            if !module_ids.contains(parent_id.as_str()) {
                return Err(AppError::Validation(format!(
                    "module {} has unknown parent {parent_id}",
                    module.id
                )));
            }
        }
        for flow in &module.flows {
            validate_flow(snapshot, module, flow)?;
        }
    }
    ordered_modules(snapshot)?;
    Ok(())
}

fn ordered_modules(snapshot: &InspectorSnapshot) -> Result<Vec<&InspectorModule>, AppError> {
    let mut ordered = Vec::with_capacity(snapshot.modules.len());
    let mut inserted = HashSet::with_capacity(snapshot.modules.len());

    while ordered.len() < snapshot.modules.len() {
        let before = ordered.len();
        for module in &snapshot.modules {
            if inserted.contains(module.id.as_str()) {
                continue;
            }
            if module
                .parent_id
                .as_deref()
                .is_none_or(|parent_id| inserted.contains(parent_id))
            {
                inserted.insert(module.id.as_str());
                ordered.push(module);
            }
        }
        if ordered.len() == before {
            return Err(AppError::Validation(
                "module parent graph contains a cycle".to_string(),
            ));
        }
    }

    Ok(ordered)
}

fn validate_flow(
    snapshot: &InspectorSnapshot,
    module: &InspectorModule,
    flow: &InspectorFlow,
) -> Result<(), AppError> {
    validate_uuid(&flow.id)?;
    if flow.project_id != snapshot.project_id
        || flow.module_id != module.id
        || flow.name.trim().is_empty()
        || flow.graph.nodes.len() > 1_000
        || flow.source_refs.len() > 1_000
    {
        return Err(AppError::Validation(format!("invalid flow {}", flow.id)));
    }
    let node_ids = flow
        .graph
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    for node in &flow.graph.nodes {
        validate_uuid(&node.id)?;
    }
    for edge in &flow.graph.edges {
        validate_uuid(&edge.id)?;
        if !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()) {
            return Err(AppError::Validation(format!(
                "flow {} has an edge with an unknown node",
                flow.id
            )));
        }
    }
    for source in &flow.source_refs {
        validate_uuid(&source.id)?;
        if source.project_id != snapshot.project_id
            || source.flow_id != flow.id
            || !valid_relative_path(&source.path)
            || source.start_line == 0
            || source.end_line < source.start_line
        {
            return Err(AppError::Validation(format!(
                "invalid source reference {}",
                source.id
            )));
        }
    }
    Ok(())
}

fn validate_uuid(value: &str) -> Result<(), AppError> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| AppError::Validation(format!("invalid UUID {value}")))
}

fn valid_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
}

fn parse_entity_status(value: &str) -> Result<EntityStatus, AppError> {
    EntityStatus::parse(value)
        .ok_or_else(|| AppError::Database(format!("unknown entity status {value}")))
}

fn usize_from_sql(value: i64) -> Result<usize, AppError> {
    usize::try_from(value).map_err(|_| AppError::Database(format!("invalid integer {value}")))
}

fn i64_from_usize(value: usize) -> Result<i64, AppError> {
    i64::try_from(value).map_err(|_| AppError::Validation(format!("integer {value} is too large")))
}
