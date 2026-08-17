use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::{error::AppError, storage::StoredProject};

const MCP_SERVER_NAME: &str = "vibe-inspector";
const MCP_BINARY_NAME: &str = "vibe-inspector-mcp";
const BACKUP_SUFFIX: &str = "vibe-inspector-backup";
const ALLOWED_PROJECTS_PLACEHOLDER: &str = "pending-registration";
const SECRET_BYTES: usize = 32;
const SESSION_SECRET_KEY: &str = "VIBE_INSPECTOR_SESSION_SECRET";
const SESSION_ID_KEY: &str = "VIBE_INSPECTOR_SESSION_ID";
const ALLOWED_PROJECTS_KEY: &str = "VIBE_INSPECTOR_ALLOWED_PROJECTS";
const ALLOW_PROJECT_CREATE_KEY: &str = "VIBE_INSPECTOR_ALLOW_PROJECT_CREATE";
const CODEX_MCP_SECTION: &str = "[mcp_servers.vibe-inspector]";
const CODEX_MCP_ENV_SECTION: &str = "[mcp_servers.vibe-inspector.env]";

const SKILL_TEMPLATES: &[SkillTemplate] = &[
    SkillTemplate {
        id: "inspector-init",
        skill_md: include_str!("../templates/skills/inspector-init/SKILL.md"),
        openai_yaml: include_str!("../templates/skills/inspector-init/agents/openai.yaml"),
    },
    SkillTemplate {
        id: "inspector-sync",
        skill_md: include_str!("../templates/skills/inspector-sync/SKILL.md"),
        openai_yaml: include_str!("../templates/skills/inspector-sync/agents/openai.yaml"),
    },
    SkillTemplate {
        id: "inspector-clarify",
        skill_md: include_str!("../templates/skills/inspector-clarify/SKILL.md"),
        openai_yaml: include_str!("../templates/skills/inspector-clarify/agents/openai.yaml"),
    },
    SkillTemplate {
        id: "inspector-review",
        skill_md: include_str!("../templates/skills/inspector-review/SKILL.md"),
        openai_yaml: include_str!("../templates/skills/inspector-review/agents/openai.yaml"),
    },
    SkillTemplate {
        id: "vibe-inspector-setup",
        skill_md: include_str!("../templates/skills/vibe-inspector-setup/SKILL.md"),
        openai_yaml: include_str!("../templates/skills/vibe-inspector-setup/agents/openai.yaml"),
    },
];

struct SkillTemplate {
    id: &'static str,
    skill_md: &'static str,
    openai_yaml: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Codex,
    Claude,
    OpenCode,
    Cursor,
    Grok,
    KimiCode,
}

impl AgentKind {
    fn all() -> &'static [AgentKind] {
        &[
            AgentKind::Codex,
            AgentKind::Claude,
            AgentKind::OpenCode,
            AgentKind::Cursor,
            AgentKind::Grok,
            AgentKind::KimiCode,
        ]
    }

    fn id(self) -> &'static str {
        match self {
            AgentKind::Codex => "codex",
            AgentKind::Claude => "claude",
            AgentKind::OpenCode => "opencode",
            AgentKind::Cursor => "cursor",
            AgentKind::Grok => "grok",
            AgentKind::KimiCode => "kimi-code",
        }
    }

    fn name(self) -> &'static str {
        match self {
            AgentKind::Codex => "Codex",
            AgentKind::Claude => "Claude Code",
            AgentKind::OpenCode => "OpenCode",
            AgentKind::Cursor => "Cursor",
            AgentKind::Grok => "Grok",
            AgentKind::KimiCode => "Kimi Code",
        }
    }

    fn binary_names(self) -> &'static [&'static str] {
        match self {
            AgentKind::Codex => &["codex"],
            AgentKind::Claude => &["claude"],
            AgentKind::OpenCode => &["opencode"],
            AgentKind::Cursor => &["cursor-agent", "cursor"],
            AgentKind::Grok => &["grok"],
            AgentKind::KimiCode => &["kimi"],
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        if id == "claude-code" {
            return Some(Self::Claude);
        }
        AgentKind::all()
            .iter()
            .copied()
            .find(|kind| kind.id() == id)
    }
}

#[derive(Debug, Clone)]
struct AgentPaths {
    kind: AgentKind,
    config_dir: PathBuf,
    skills_dir: PathBuf,
    mcp_config_path: PathBuf,
    requires_home_containment: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SkillInstallStatus {
    Installed,
    Outdated,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillState {
    pub id: String,
    pub status: SkillInstallStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegration {
    pub id: String,
    pub name: String,
    pub detected: bool,
    pub detection_evidence: Vec<String>,
    pub skills_dir: String,
    pub skill_states: Vec<SkillState>,
    pub mcp_configured: bool,
    pub mcp_config_path: String,
}

impl AgentIntegration {
    pub fn needs_install(&self) -> bool {
        !self.mcp_configured
            || self
                .skill_states
                .iter()
                .any(|skill| skill.status != SkillInstallStatus::Installed)
    }
}

/// Per-agent install outcome: a failed agent yields an error item without
/// interrupting the remaining agents.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationResult {
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<AgentIntegration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AppError>,
}

impl AgentIntegrationResult {
    fn installed(integration: AgentIntegration) -> Self {
        Self {
            agent_id: integration.id.clone(),
            integration: Some(integration),
            error: None,
        }
    }

    fn failed(agent_id: impl Into<String>, error: AppError) -> Self {
        Self {
            agent_id: agent_id.into(),
            integration: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallAgentIntegrationsInput {
    pub agent_ids: Vec<String>,
}

/// Resolves the on-disk layout of every supported coding agent from a home
/// directory. Optional home overrides mirror the agents' documented
/// environment variables.
fn agent_paths(
    home: &Path,
    kimi_code_home: Option<&Path>,
    xdg_config_home: Option<&Path>,
    grok_home: Option<&Path>,
) -> Vec<AgentPaths> {
    AgentKind::all()
        .iter()
        .map(|kind| match kind {
            AgentKind::Codex => {
                let config_dir = home.join(".codex");
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: config_dir.join("config.toml"),
                    config_dir,
                    requires_home_containment: true,
                }
            }
            AgentKind::Claude => {
                let config_dir = home.join(".claude");
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: home.join(".claude.json"),
                    config_dir,
                    requires_home_containment: true,
                }
            }
            AgentKind::OpenCode => {
                let config_dir = xdg_config_home
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| home.join(".config"))
                    .join("opencode");
                let json = config_dir.join("opencode.json");
                let jsonc = config_dir.join("opencode.jsonc");
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: if jsonc.exists() || !json.exists() {
                        jsonc
                    } else {
                        json
                    },
                    config_dir,
                    requires_home_containment: xdg_config_home.is_none(),
                }
            }
            AgentKind::Cursor => {
                let config_dir = home.join(".cursor");
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: config_dir.join("mcp.json"),
                    config_dir,
                    requires_home_containment: true,
                }
            }
            AgentKind::Grok => {
                let config_dir = grok_home
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| home.join(".grok"));
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: config_dir.join("config.toml"),
                    config_dir,
                    requires_home_containment: grok_home.is_none(),
                }
            }
            AgentKind::KimiCode => {
                let config_dir = kimi_code_home
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| home.join(".kimi-code"));
                AgentPaths {
                    kind: *kind,
                    skills_dir: config_dir.join("skills"),
                    mcp_config_path: config_dir.join("mcp.json"),
                    config_dir,
                    requires_home_containment: kimi_code_home.is_none(),
                }
            }
        })
        .collect()
}

/// Detects which agents are installed on this machine. A directory under
/// `home` or a binary in `path_entries` both count as evidence.
fn detect_agents(
    home: &Path,
    path_entries: &[PathBuf],
    kimi_code_home: Option<&Path>,
    xdg_config_home: Option<&Path>,
    grok_home: Option<&Path>,
) -> Vec<AgentIntegration> {
    agent_paths(home, kimi_code_home, xdg_config_home, grok_home)
        .into_iter()
        .map(|paths| {
            let evidence = detection_evidence(&paths, home, path_entries);
            build_integration(&paths, !evidence.is_empty(), evidence)
        })
        .collect()
}

fn detection_evidence(paths: &AgentPaths, home: &Path, path_entries: &[PathBuf]) -> Vec<String> {
    let mut evidence = Vec::new();
    if paths.config_dir.is_dir() {
        evidence.push(paths.config_dir.display().to_string());
    }
    'binary: for binary in paths.kind.binary_names() {
        for entry in path_entries {
            let candidate = entry.join(binary);
            if is_executable_file(&candidate) {
                evidence.push(candidate.display().to_string());
                break 'binary;
            }
        }
    }
    if paths.kind == AgentKind::Cursor {
        for application in [
            PathBuf::from("/Applications/Cursor.app"),
            home.join("Applications/Cursor.app"),
        ] {
            if application.is_dir() {
                evidence.push(application.display().to_string());
            }
        }
    }
    evidence
}

fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn build_integration(
    paths: &AgentPaths,
    detected: bool,
    detection_evidence: Vec<String>,
) -> AgentIntegration {
    AgentIntegration {
        id: paths.kind.id().to_string(),
        name: paths.kind.name().to_string(),
        detected,
        detection_evidence,
        skills_dir: paths.skills_dir.display().to_string(),
        skill_states: skill_states(paths),
        mcp_configured: mcp_configured(paths),
        mcp_config_path: paths.mcp_config_path.display().to_string(),
    }
}

fn skill_states(paths: &AgentPaths) -> Vec<SkillState> {
    SKILL_TEMPLATES
        .iter()
        .map(|template| {
            let skill_dir = paths.skills_dir.join(template.id);
            let status = match fs::read_to_string(skill_dir.join("SKILL.md")) {
                Ok(content) if content == template.skill_md => {
                    let codex_metadata_current = paths.kind != AgentKind::Codex
                        || fs::read_to_string(skill_dir.join("agents/openai.yaml"))
                            .is_ok_and(|content| content == template.openai_yaml);
                    if codex_metadata_current {
                        SkillInstallStatus::Installed
                    } else {
                        SkillInstallStatus::Outdated
                    }
                }
                Ok(_) => SkillInstallStatus::Outdated,
                Err(_) => SkillInstallStatus::Missing,
            };
            SkillState {
                id: template.id.to_string(),
                status,
            }
        })
        .collect()
}

fn mcp_configured(paths: &AgentPaths) -> bool {
    let Ok(content) = fs::read_to_string(&paths.mcp_config_path) else {
        return false;
    };
    match paths.kind {
        AgentKind::Codex | AgentKind::Grok => {
            let Ok(document) = toml::from_str::<toml::Table>(&content) else {
                return false;
            };
            let Some(server) = document
                .get("mcp_servers")
                .and_then(|servers| servers.get(MCP_SERVER_NAME))
                .and_then(toml::Value::as_table)
            else {
                return false;
            };
            let Some(command) = server.get("command").and_then(toml::Value::as_str) else {
                return false;
            };
            let Some(environment) = server.get("env").and_then(toml::Value::as_table) else {
                return false;
            };
            command_is_present(command)
                && toml_environment_is_configured(
                    environment,
                    &format!("agent-{}", paths.kind.id()),
                )
        }
        AgentKind::Claude | AgentKind::Cursor | AgentKind::KimiCode => {
            serde_json::from_str::<Value>(&content)
                .ok()
                .and_then(|document| {
                    let server = document.get("mcpServers")?.get(MCP_SERVER_NAME)?;
                    let command = server.get("command")?.as_str()?;
                    let environment = server.get("env")?;
                    (command_is_present(command)
                        && (paths.kind != AgentKind::Cursor
                            || server.get("type").and_then(Value::as_str) == Some("stdio"))
                        && json_environment_is_configured(
                            environment,
                            &format!("agent-{}", paths.kind.id()),
                        ))
                    .then_some(())
                })
                .is_some()
        }
        AgentKind::OpenCode => parse_jsonc(&content)
            .ok()
            .and_then(|document| {
                let server = document.get("mcp")?.get(MCP_SERVER_NAME)?;
                let command = server.get("command")?.as_array()?;
                let command = command.first()?.as_str()?;
                let environment = server.get("environment")?;
                (server.get("type").and_then(Value::as_str) == Some("local")
                    && server.get("enabled").and_then(Value::as_bool) == Some(true)
                    && command_is_present(command)
                    && json_environment_is_configured(
                        environment,
                        &format!("agent-{}", paths.kind.id()),
                    ))
                .then_some(())
            })
            .is_some(),
    }
}

fn mcp_entry_present(paths: &AgentPaths) -> bool {
    let Ok(content) = fs::read_to_string(&paths.mcp_config_path) else {
        return false;
    };
    match paths.kind {
        AgentKind::Codex | AgentKind::Grok => has_codex_mcp_section(&content).unwrap_or(false),
        AgentKind::Claude | AgentKind::Cursor | AgentKind::KimiCode => {
            serde_json::from_str::<Value>(&content)
                .ok()
                .and_then(|document| {
                    document
                        .get("mcpServers")?
                        .get(MCP_SERVER_NAME)?
                        .as_object()
                        .map(|_| ())
                })
                .is_some()
        }
        AgentKind::OpenCode => parse_jsonc(&content)
            .ok()
            .and_then(|document| {
                document
                    .get("mcp")?
                    .get(MCP_SERVER_NAME)?
                    .as_object()
                    .map(|_| ())
            })
            .is_some(),
    }
}

fn command_is_present(command: &str) -> bool {
    !command.trim().is_empty()
}

fn valid_session_secret(secret: &str) -> bool {
    secret.len() == SECRET_BYTES * 2 && secret.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn json_environment_is_configured(environment: &Value, expected_session_id: &str) -> bool {
    let Some(environment) = environment.as_object() else {
        return false;
    };
    let Some(secret) = environment.get(SESSION_SECRET_KEY).and_then(Value::as_str) else {
        return false;
    };
    let Some(session_id) = environment.get(SESSION_ID_KEY).and_then(Value::as_str) else {
        return false;
    };
    let Some(allowed_projects) = environment
        .get(ALLOWED_PROJECTS_KEY)
        .and_then(Value::as_str)
    else {
        return false;
    };
    environment
        .get(ALLOW_PROJECT_CREATE_KEY)
        .and_then(Value::as_str)
        == Some("1")
        && valid_session_secret(secret)
        && session_id == expected_session_id
        && !allowed_projects.trim().is_empty()
}

fn toml_environment_is_configured(environment: &toml::Table, expected_session_id: &str) -> bool {
    let Some(secret) = environment
        .get(SESSION_SECRET_KEY)
        .and_then(toml::Value::as_str)
    else {
        return false;
    };
    let Some(session_id) = environment
        .get(SESSION_ID_KEY)
        .and_then(toml::Value::as_str)
    else {
        return false;
    };
    let Some(allowed_projects) = environment
        .get(ALLOWED_PROJECTS_KEY)
        .and_then(toml::Value::as_str)
    else {
        return false;
    };
    environment
        .get(ALLOW_PROJECT_CREATE_KEY)
        .and_then(toml::Value::as_str)
        == Some("1")
        && valid_session_secret(secret)
        && session_id == expected_session_id
        && !allowed_projects.trim().is_empty()
}

fn parse_jsonc(content: &str) -> Result<Value, AppError> {
    json5::from_str::<Value>(content).map_err(|error| {
        AppError::Validation(format!("OpenCode config is not valid JSONC: {error}"))
    })
}

/// Merges a `mcpServers["vibe-inspector"]` entry into a JSON config document,
/// preserving every unknown field. Re-running with the same entry yields no
/// diff.
fn merge_json_mcp_config(
    document: &Value,
    command: &str,
    env: &BTreeMap<String, String>,
) -> Result<Value, AppError> {
    let mut root = match document {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        _ => {
            return Err(AppError::Validation(
                "agent MCP config must be a JSON object".to_string(),
            ))
        }
    };
    let servers = root
        .entry("mcpServers".to_string())
        .or_insert_with(|| json!({}));
    if !servers.is_object() {
        return Err(AppError::Validation(
            "agent MCP config field mcpServers must be an object".to_string(),
        ));
    }
    let server = if let Some(value) = servers.get_mut(MCP_SERVER_NAME) {
        match value {
            Value::Object(map) => map,
            _ => {
                return Err(AppError::Validation(
                    "agent MCP server entry must be an object".to_string(),
                ))
            }
        }
    } else {
        let servers = servers
            .as_object_mut()
            .expect("mcpServers object was checked above");
        servers
            .entry(MCP_SERVER_NAME.to_string())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("new MCP server entry is an object")
    };
    server.insert("command".to_string(), Value::String(command.to_string()));
    merge_agent_environment(server, "env", env)?;
    Ok(Value::Object(root))
}

/// Merges OpenCode's local MCP shape into an `opencode.json` or
/// `opencode.jsonc` document while preserving every unknown field.
fn merge_opencode_mcp_config(
    document: &Value,
    command: &str,
    env: &BTreeMap<String, String>,
) -> Result<Value, AppError> {
    let mut root = match document {
        Value::Null => Map::new(),
        Value::Object(map) => map.clone(),
        _ => {
            return Err(AppError::Validation(
                "OpenCode config must be a JSON object".to_string(),
            ))
        }
    };
    let servers = root.entry("mcp".to_string()).or_insert_with(|| json!({}));
    if !servers.is_object() {
        return Err(AppError::Validation(
            "OpenCode config field mcp must be an object".to_string(),
        ));
    }
    let server = if let Some(value) = servers.get_mut(MCP_SERVER_NAME) {
        match value {
            Value::Object(map) => map,
            _ => {
                return Err(AppError::Validation(
                    "OpenCode MCP server entry must be an object".to_string(),
                ))
            }
        }
    } else {
        let servers = servers
            .as_object_mut()
            .expect("mcp object was checked above");
        servers
            .entry(MCP_SERVER_NAME.to_string())
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .expect("new OpenCode MCP server entry is an object")
    };
    server.insert("type".to_string(), Value::String("local".to_string()));
    server.insert(
        "command".to_string(),
        Value::Array(vec![Value::String(command.to_string())]),
    );
    merge_agent_environment(server, "environment", env)?;
    server.insert("enabled".to_string(), Value::Bool(true));
    Ok(Value::Object(root))
}

fn merge_agent_environment(
    server: &mut Map<String, Value>,
    field: &str,
    proposed: &BTreeMap<String, String>,
) -> Result<(), AppError> {
    let environment = server.entry(field.to_string()).or_insert_with(|| json!({}));
    let environment = environment.as_object_mut().ok_or_else(|| {
        AppError::Validation(format!("agent MCP server field {field} must be an object"))
    })?;
    for (key, value) in proposed {
        let keep_existing_identity = match key.as_str() {
            SESSION_SECRET_KEY => environment
                .get(key)
                .and_then(Value::as_str)
                .is_some_and(valid_session_secret),
            SESSION_ID_KEY => environment.get(key).and_then(Value::as_str) == Some(value),
            _ => false,
        };
        if !keep_existing_identity {
            environment.insert(key.clone(), Value::String(value.clone()));
        }
    }
    Ok(())
}

/// Checks whether `mcp_servers.vibe-inspector` exists by parsing the TOML
/// document, so quoted keys, spacing variants, and inline tables all count.
/// A corrupt file is an error instead of an excuse to append a duplicate.
fn has_codex_mcp_section(config_text: &str) -> Result<bool, AppError> {
    let document = toml::from_str::<toml::Table>(config_text).map_err(|error| {
        AppError::Validation(format!("codex config.toml is not valid TOML: {error}"))
    })?;
    Ok(document
        .get("mcp_servers")
        .and_then(|servers| servers.get(MCP_SERVER_NAME))
        .is_some())
}

/// Renders the TOML block appended to `~/.codex/config.toml`.
fn codex_mcp_block(command: &str, env: &BTreeMap<String, String>) -> String {
    let mut block = format!("{CODEX_MCP_SECTION}\ncommand = {}\n", toml_string(command));
    block.push_str(CODEX_MCP_ENV_SECTION);
    block.push('\n');
    for (key, value) in env {
        block.push_str(&format!("{key} = {}\n", toml_string(value)));
    }
    block
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

/// Adds the VibeInspector MCP server to a Codex config. Without an existing
/// section the block is appended; with one, `command` and
/// `VIBE_INSPECTOR_ALLOWED_PROJECTS` are updated in place while a valid session
/// secret and the expected session ID are preserved. Invalid identity values
/// are repaired. An empty project list leaves the allow-list untouched.
fn upsert_codex_mcp_config(
    existing: &str,
    command: &str,
    env: &BTreeMap<String, String>,
    project_ids: &[String],
) -> Result<String, AppError> {
    if !has_codex_mcp_section(existing)? {
        let mut updated = existing.to_string();
        if !updated.is_empty() && !updated.ends_with('\n') {
            updated.push('\n');
        }
        if !updated.is_empty() {
            updated.push('\n');
        }
        updated.push_str(&codex_mcp_block(command, env));
        return Ok(updated);
    }
    let allowed = if project_ids.is_empty() {
        None
    } else {
        Some(project_ids.join(","))
    };
    update_codex_mcp_config_with_identity(existing, Some(command), allowed.as_deref(), env)
}

/// Normalizes a TOML section header so `[mcp_servers."vibe-inspector"]` and
/// `[ mcp_servers.vibe-inspector ]` both match our block.
fn section_header_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('[') || trimmed.starts_with("[[") {
        return None;
    }
    let closing = trimmed.find(']')?;
    let suffix = trimmed[closing + 1..].trim();
    if !suffix.is_empty() && !suffix.starts_with('#') {
        return None;
    }
    let inner = &trimmed[1..closing];
    Some(
        inner
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
            .collect(),
    )
}

/// Extracts the (possibly quoted) key of a `key = value` TOML line.
fn toml_line_key(line: &str) -> Option<&str> {
    let (key, _) = line.split_once('=')?;
    Some(key.trim().trim_matches('"').trim_matches('\''))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodexSection {
    Server,
    Env,
    Other,
}

/// Rewrites `command` (in `[mcp_servers.vibe-inspector]`) and the managed env
/// keys (`VIBE_INSPECTOR_ALLOWED_PROJECTS` when supplied, plus
/// `VIBE_INSPECTOR_ALLOW_PROJECT_CREATE`, so previously installed agents gain
/// auto-import on the next sync) in the `.env` sub-section in place.
/// Keys are matched exactly after splitting on `=`, missing keys are
/// inserted, and every other line is preserved. Identity values are only
/// changed by the identity-aware wrapper. `None` leaves a value untouched.
fn update_codex_mcp_config(
    existing: &str,
    command: Option<&str>,
    allowed_projects: Option<&str>,
) -> Result<String, AppError> {
    update_codex_mcp_config_with_identity(existing, command, allowed_projects, &BTreeMap::new())
}

fn update_codex_mcp_config_with_identity(
    existing: &str,
    command: Option<&str>,
    allowed_projects: Option<&str>,
    identity: &BTreeMap<String, String>,
) -> Result<String, AppError> {
    let existing_document = toml::from_str::<toml::Table>(existing).ok();
    let existing_environment = existing_document
        .as_ref()
        .and_then(|document| document.get("mcp_servers"))
        .and_then(|servers| servers.get(MCP_SERVER_NAME))
        .and_then(|server| server.get("env"))
        .and_then(toml::Value::as_table);
    let mut env_updates: Vec<(&str, &str, bool)> = allowed_projects
        .map(|allowed| (ALLOWED_PROJECTS_KEY, allowed, true))
        .into_iter()
        .collect();
    env_updates.push((ALLOW_PROJECT_CREATE_KEY, "1", true));
    for key in [SESSION_SECRET_KEY, SESSION_ID_KEY] {
        if let Some(value) = identity.get(key) {
            let overwrite_existing = match key {
                SESSION_SECRET_KEY => !existing_environment
                    .and_then(|environment| environment.get(key))
                    .and_then(toml::Value::as_str)
                    .is_some_and(valid_session_secret),
                SESSION_ID_KEY => {
                    existing_environment
                        .and_then(|environment| environment.get(key))
                        .and_then(toml::Value::as_str)
                        != Some(value)
                }
                _ => false,
            };
            env_updates.push((key, value, overwrite_existing));
        }
    }
    let mut pending: Vec<usize> = (0..env_updates.len()).collect();
    let mut lines: Vec<String> = existing.lines().map(str::to_string).collect();
    let mut section = CodexSection::Other;
    let mut command_done = command.is_none();
    let mut server_header = None;
    let mut env_header = None;
    let mut env_inline = false;
    for (index, line) in lines.iter_mut().enumerate() {
        if let Some(header) = section_header_key(line) {
            section = match header.as_str() {
                "mcp_servers.vibe-inspector" => {
                    server_header = Some(index);
                    CodexSection::Server
                }
                "mcp_servers.vibe-inspector.env" => {
                    env_header = Some(index);
                    CodexSection::Env
                }
                _ => CodexSection::Other,
            };
            continue;
        }
        match section {
            CodexSection::Server => {
                if !command_done && toml_line_key(line) == Some("command") {
                    let command = command.expect("command update pending");
                    *line = format!("command = {}", toml_string(command));
                    command_done = true;
                } else if toml_line_key(line) == Some("env") {
                    env_inline = true;
                }
            }
            CodexSection::Env => {
                let matching = toml_line_key(line).and_then(|key| {
                    pending
                        .iter()
                        .position(|update| env_updates[*update].0 == key)
                });
                if let Some(position) = matching {
                    let update = pending.remove(position);
                    let (key, value, overwrite_existing) = env_updates[update];
                    if overwrite_existing {
                        *line = format!("{key} = {}", toml_string(value));
                    }
                }
            }
            CodexSection::Other => {}
        }
    }
    if !command_done {
        let command = command.expect("command update pending");
        let Some(header) = server_header else {
            return Err(AppError::Validation(
                "codex config.toml declares mcp_servers.vibe-inspector without a writable section header"
                    .to_string(),
            ));
        };
        lines.insert(header + 1, format!("command = {}", toml_string(command)));
        if let Some(env_index) = env_header {
            env_header = Some(env_index + 1);
        }
    }
    if !pending.is_empty() {
        let missing = pending
            .iter()
            .map(|update| {
                let (key, value, _) = env_updates[*update];
                format!("{key} = {}", toml_string(value))
            })
            .collect::<Vec<_>>();
        if let Some(header) = env_header {
            for (offset, line) in missing.into_iter().enumerate() {
                lines.insert(header + 1 + offset, line);
            }
        } else if env_inline {
            return Err(AppError::Validation(
                "codex config.toml declares the vibe-inspector env as an inline table".to_string(),
            ));
        } else {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push(CODEX_MCP_ENV_SECTION.to_string());
            lines.extend(missing);
        }
    }
    let mut text = lines.join("\n");
    if existing.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn mcp_env(session_id: &str, secret: &str, project_ids: &[String]) -> BTreeMap<String, String> {
    let allowed = if project_ids.is_empty() {
        ALLOWED_PROJECTS_PLACEHOLDER.to_string()
    } else {
        project_ids.join(",")
    };
    BTreeMap::from([
        (SESSION_SECRET_KEY.to_string(), secret.to_string()),
        (SESSION_ID_KEY.to_string(), session_id.to_string()),
        (ALLOWED_PROJECTS_KEY.to_string(), allowed),
        (ALLOW_PROJECT_CREATE_KEY.to_string(), "1".to_string()),
    ])
}

/// Unions registry project IDs with every active Inspector project ID.
/// Projects created through MCP never enter the JSON registry, so syncing
/// from the registry alone would strip them from every agent configuration.
pub fn merged_project_ids(registry_ids: &[String], stored: &[StoredProject]) -> Vec<String> {
    let mut ids = registry_ids.to_vec();
    for project in stored {
        if !ids.contains(&project.id) {
            ids.push(project.id.clone());
        }
    }
    ids
}

fn generate_session_secret() -> Result<String, AppError> {
    let mut bytes = [0_u8; SECRET_BYTES];
    getrandom::fill(&mut bytes)
        .map_err(|error| AppError::Storage(format!("OS random source failed: {error}")))?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(SECRET_BYTES * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(encoded)
}

pub fn mcp_binary_path() -> Result<PathBuf, AppError> {
    if let Some(path) = std::env::var_os("VIBE_INSPECTOR_MCP_PATH") {
        return validate_mcp_binary(PathBuf::from(path));
    }
    let executable = std::env::current_exe()
        .map_err(|error| AppError::Storage(format!("current executable is unknown: {error}")))?;
    let directory = executable.parent().ok_or_else(|| {
        AppError::Storage("current executable has no parent directory".to_string())
    })?;
    let binary = directory.join(MCP_BINARY_NAME);
    if binary.is_file() {
        return validate_mcp_binary(binary);
    }
    for entry in path_entries() {
        let candidate = entry.join(MCP_BINARY_NAME);
        if candidate.is_file() {
            return validate_mcp_binary(candidate);
        }
    }
    let mut bundled_candidates =
        vec![PathBuf::from("/Applications/VibeInspector.app/Contents/MacOS").join(MCP_BINARY_NAME)];
    if let Ok(home) = home_dir() {
        bundled_candidates.push(
            home.join("Applications/VibeInspector.app/Contents/MacOS")
                .join(MCP_BINARY_NAME),
        );
    }
    for candidate in bundled_candidates {
        if candidate.is_file() {
            return validate_mcp_binary(candidate);
        }
    }
    Err(AppError::NotFound(format!(
        "{MCP_BINARY_NAME} is not next to {}, on PATH, or inside VibeInspector.app",
        executable.display()
    )))
}

fn validate_mcp_binary(path: PathBuf) -> Result<PathBuf, AppError> {
    let resolved = path
        .canonicalize()
        .map_err(|error| AppError::InvalidPath(format!("{}: {error}", path.display())))?;
    if !resolved.is_file() {
        return Err(AppError::NotFound(resolved.display().to_string()));
    }
    if !is_executable_file(&resolved) {
        return Err(AppError::Validation(format!(
            "MCP helper is not executable: {}",
            resolved.display()
        )));
    }
    Ok(resolved)
}

/// Guards every write: the target must resolve inside `containment_dir`, the
/// agent's config directory (or the home directory for Claude, whose MCP
/// config lives at `~/.claude.json`). Existence is probed with
/// `symlink_metadata`, so a dangling symlink fails the canonicalize step
/// instead of passing the containment check as a "new file" and letting a
/// later write follow it outside.
fn ensure_write_allowed(path: &Path, containment_dir: &Path) -> Result<PathBuf, AppError> {
    let resolved_dir = containment_dir.canonicalize().map_err(|error| {
        AppError::InvalidPath(format!("{}: {error}", containment_dir.display()))
    })?;
    let resolved = match fs::symlink_metadata(path) {
        Ok(_) => path
            .canonicalize()
            .map_err(|error| AppError::InvalidPath(format!("{}: {error}", path.display())))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or_else(|| {
                AppError::InvalidPath(format!("{} has no parent directory", path.display()))
            })?;
            let resolved_parent = parent
                .canonicalize()
                .map_err(|error| AppError::InvalidPath(format!("{}: {error}", parent.display())))?;
            let name = path.file_name().ok_or_else(|| {
                AppError::InvalidPath(format!("{} has no file name", path.display()))
            })?;
            resolved_parent.join(name)
        }
        Err(error) => {
            return Err(AppError::InvalidPath(format!(
                "{}: {error}",
                path.display()
            )))
        }
    };
    if !resolved.starts_with(&resolved_dir) {
        return Err(AppError::PathTraversal(path.display().to_string()));
    }
    Ok(resolved)
}

fn ensure_directory_allowed(path: &Path, containment_dir: Option<&Path>) -> Result<(), AppError> {
    let resolved_containment = containment_dir
        .map(|directory| {
            directory
                .canonicalize()
                .map_err(|error| AppError::InvalidPath(format!("{}: {error}", directory.display())))
        })
        .transpose()?;

    if let Some(directory) = containment_dir {
        if !path.starts_with(directory) {
            return Err(AppError::PathTraversal(path.display().to_string()));
        }
        let mut existing = path;
        loop {
            match fs::symlink_metadata(existing) {
                Ok(_) => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    existing = existing.parent().ok_or_else(|| {
                        AppError::InvalidPath(format!(
                            "{} has no existing ancestor",
                            path.display()
                        ))
                    })?;
                }
                Err(error) => {
                    return Err(AppError::InvalidPath(format!(
                        "{}: {error}",
                        existing.display()
                    )))
                }
            }
        }
        let resolved_existing = existing
            .canonicalize()
            .map_err(|error| AppError::InvalidPath(format!("{}: {error}", existing.display())))?;
        if !resolved_existing.starts_with(
            resolved_containment
                .as_ref()
                .expect("resolved when containment is present"),
        ) {
            return Err(AppError::PathTraversal(path.display().to_string()));
        }
    }

    fs::create_dir_all(path).map_err(storage_error)?;
    validate_directory_allowed(path, resolved_containment.as_deref())
}

fn validate_directory_allowed(path: &Path, containment_dir: Option<&Path>) -> Result<(), AppError> {
    let resolved = path
        .canonicalize()
        .map_err(|error| AppError::InvalidPath(format!("{}: {error}", path.display())))?;
    if !resolved.is_dir() {
        return Err(AppError::InvalidPath(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    if let Some(containment) = containment_dir {
        let resolved_containment = containment.canonicalize().map_err(|error| {
            AppError::InvalidPath(format!("{}: {error}", containment.display()))
        })?;
        if !resolved.starts_with(resolved_containment) {
            return Err(AppError::PathTraversal(path.display().to_string()));
        }
    }
    Ok(())
}

fn backup_existing(path: &Path) -> Result<(), AppError> {
    if fs::symlink_metadata(path).is_err() {
        return Ok(());
    }
    let backup = PathBuf::from(format!("{}.{BACKUP_SUFFIX}", path.display()));
    if fs::symlink_metadata(&backup).is_ok() {
        // Keep the first backup so the pre-VibeInspector original survives.
        return Ok(());
    }
    fs::copy(path, &backup).map_err(|error| {
        AppError::Storage(format!("backup of {} failed: {error}", path.display()))
    })?;
    // fs::copy inherits the source permissions, which may be world-readable.
    fs::set_permissions(&backup, fs::Permissions::from_mode(0o600)).map_err(storage_error)
}

fn write_private_file(path: &Path, contents: &str) -> Result<(), AppError> {
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(storage_error)?;
    file.write_all(contents.as_bytes()).map_err(storage_error)?;
    file.sync_all().map_err(storage_error)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(storage_error)
}

fn storage_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(error.to_string())
}

fn install_skills(paths: &AgentPaths) -> Result<(), AppError> {
    ensure_directory_allowed(&paths.skills_dir, Some(&paths.config_dir))?;
    for state in skill_states(paths) {
        if state.status == SkillInstallStatus::Installed {
            continue;
        }
        let template = SKILL_TEMPLATES
            .iter()
            .find(|template| template.id == state.id)
            .expect("skill state originates from the template list");
        let skill_dir = paths.skills_dir.join(template.id);
        ensure_directory_allowed(&skill_dir, Some(&paths.skills_dir))?;
        let skill_path = ensure_write_allowed(&skill_dir.join("SKILL.md"), &skill_dir)?;
        fs::write(&skill_path, template.skill_md).map_err(storage_error)?;
        if paths.kind == AgentKind::Codex {
            let agents_dir = skill_dir.join("agents");
            ensure_directory_allowed(&agents_dir, Some(&skill_dir))?;
            let yaml_path = ensure_write_allowed(&agents_dir.join("openai.yaml"), &skill_dir)?;
            fs::write(&yaml_path, template.openai_yaml).map_err(storage_error)?;
        }
    }
    Ok(())
}

fn install_mcp_config(
    paths: &AgentPaths,
    home: &Path,
    command: &str,
    env: &BTreeMap<String, String>,
    project_ids: &[String],
) -> Result<(), AppError> {
    match paths.kind {
        AgentKind::Codex | AgentKind::Grok => {
            let target = ensure_write_allowed(&paths.mcp_config_path, &paths.config_dir)?;
            let existing = fs::read_to_string(&target).unwrap_or_default();
            let updated = upsert_codex_mcp_config(&existing, command, env, project_ids)?;
            if updated == existing {
                return Ok(());
            }
            backup_existing(&target)?;
            write_private_file(&target, &updated)
        }
        AgentKind::Claude | AgentKind::Cursor | AgentKind::KimiCode => {
            let existed = fs::symlink_metadata(&paths.mcp_config_path).is_ok();
            let target =
                ensure_write_allowed(&paths.mcp_config_path, mcp_containment(paths, home))?;
            let document = if existed {
                let text = fs::read_to_string(&target).map_err(storage_error)?;
                serde_json::from_str::<Value>(&text).map_err(|error| {
                    AppError::Validation(format!(
                        "{} is not valid JSON: {error}",
                        paths.mcp_config_path.display()
                    ))
                })?
            } else {
                Value::Null
            };
            let mut merged = merge_json_mcp_config(&document, command, env)?;
            if paths.kind == AgentKind::Cursor {
                merged["mcpServers"][MCP_SERVER_NAME]["type"] = Value::String("stdio".to_string());
            }
            if existed {
                let merged_text = serde_json::to_string_pretty(&merged).map_err(storage_error)?;
                let current = fs::read_to_string(&target).map_err(storage_error)?;
                if current == merged_text {
                    return Ok(());
                }
                backup_existing(&target)?;
                write_private_file(&target, &merged_text)
            } else {
                write_private_file(
                    &target,
                    &serde_json::to_string_pretty(&merged).map_err(storage_error)?,
                )
            }
        }
        AgentKind::OpenCode => {
            let existed = fs::symlink_metadata(&paths.mcp_config_path).is_ok();
            let target = ensure_write_allowed(&paths.mcp_config_path, &paths.config_dir)?;
            let document = if existed {
                let text = fs::read_to_string(&target).map_err(storage_error)?;
                parse_jsonc(&text)?
            } else {
                Value::Null
            };
            let merged = merge_opencode_mcp_config(&document, command, env)?;
            let updated = serde_json::to_string_pretty(&merged).map_err(storage_error)?;
            if existed {
                let current = fs::read_to_string(&target).map_err(storage_error)?;
                if parse_jsonc(&current)? == merged {
                    return Ok(());
                }
                backup_existing(&target)?;
            }
            write_private_file(&target, &updated)
        }
    }
}

/// Claude keeps its MCP config at `~/.claude.json`, outside its config
/// directory, so the write guard uses the home directory as containment.
fn mcp_containment<'a>(paths: &'a AgentPaths, home: &'a Path) -> &'a Path {
    match paths.kind {
        AgentKind::Claude => home,
        AgentKind::Codex
        | AgentKind::OpenCode
        | AgentKind::Cursor
        | AgentKind::Grok
        | AgentKind::KimiCode => &paths.config_dir,
    }
}

fn install_agent(
    paths: &AgentPaths,
    home: &Path,
    command: &str,
    project_ids: &[String],
) -> Result<AgentIntegration, AppError> {
    ensure_directory_allowed(
        &paths.config_dir,
        paths.requires_home_containment.then_some(home),
    )?;
    install_skills(paths)?;
    let secret = generate_session_secret()?;
    let env = mcp_env(&format!("agent-{}", paths.kind.id()), &secret, project_ids);
    install_mcp_config(paths, home, command, &env, project_ids)?;
    let evidence = vec![paths.config_dir.display().to_string()];
    Ok(build_integration(paths, true, evidence))
}

/// Updates `VIBE_INSPECTOR_ALLOWED_PROJECTS` in every already-configured
/// agent MCP config. Runs best-effort after project registration; failures
/// are logged and never block registration. `project_ids` should come from
/// `merged_project_ids` so MCP-created projects survive the sync.
pub fn sync_agent_project_allowlist(project_ids: &[String]) {
    if let Err(error) = sync_agent_project_allowlist_inner(project_ids) {
        eprintln!("vibe-inspector: agent allow-list sync failed: {error}");
    }
}

fn sync_agent_project_allowlist_inner(project_ids: &[String]) -> Result<(), AppError> {
    let home = PathBuf::from(
        std::env::var_os("HOME")
            .ok_or_else(|| AppError::Storage("HOME is unavailable".to_string()))?,
    );
    let kimi_code_home = std::env::var_os("KIMI_CODE_HOME").map(PathBuf::from);
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let grok_home = std::env::var_os("GROK_HOME").map(PathBuf::from);
    for paths in agent_paths(
        &home,
        kimi_code_home.as_deref(),
        xdg_config_home.as_deref(),
        grok_home.as_deref(),
    ) {
        if let Err(error) = sync_agent_allowlist(&paths, &home, project_ids) {
            eprintln!(
                "vibe-inspector: allow-list sync for {} failed: {error}",
                paths.kind.id()
            );
        }
    }
    Ok(())
}

fn sync_agent_allowlist(
    paths: &AgentPaths,
    home: &Path,
    project_ids: &[String],
) -> Result<(), AppError> {
    if project_ids.is_empty() {
        // Never write an empty allow-list; the MCP server would refuse to
        // start. Keep the existing value (or the install-time placeholder).
        return Ok(());
    }
    if fs::symlink_metadata(&paths.config_dir).is_ok() {
        validate_directory_allowed(
            &paths.config_dir,
            paths.requires_home_containment.then_some(home),
        )?;
    }
    if !mcp_entry_present(paths) {
        return Ok(());
    }
    let target = ensure_write_allowed(&paths.mcp_config_path, mcp_containment(paths, home))?;
    let existing = fs::read_to_string(&target).map_err(storage_error)?;
    match paths.kind {
        AgentKind::Codex | AgentKind::Grok => {
            let updated = update_codex_mcp_config(&existing, None, Some(&project_ids.join(",")))?;
            if updated == existing {
                return Ok(());
            }
            backup_existing(&target)?;
            write_private_file(&target, &updated)
        }
        AgentKind::Claude | AgentKind::Cursor | AgentKind::KimiCode => {
            let mut document = serde_json::from_str::<Value>(&existing).map_err(|error| {
                AppError::Validation(format!(
                    "{} is not valid JSON: {error}",
                    paths.mcp_config_path.display()
                ))
            })?;
            let Some(server) = document
                .get_mut("mcpServers")
                .and_then(|servers| servers.get_mut(MCP_SERVER_NAME))
            else {
                return Ok(());
            };
            let Some(env) = server.get_mut("env").and_then(Value::as_object_mut) else {
                return Ok(());
            };
            env.insert(
                "VIBE_INSPECTOR_ALLOWED_PROJECTS".to_string(),
                Value::String(project_ids.join(",")),
            );
            // Previously installed agents predate project auto-import; make
            // sure every sync upgrades them to allow it.
            env.insert(
                "VIBE_INSPECTOR_ALLOW_PROJECT_CREATE".to_string(),
                Value::String("1".to_string()),
            );
            let updated = serde_json::to_string_pretty(&document).map_err(storage_error)?;
            if updated == existing {
                return Ok(());
            }
            backup_existing(&target)?;
            write_private_file(&target, &updated)
        }
        AgentKind::OpenCode => {
            let mut document = parse_jsonc(&existing)?;
            let Some(environment) = document
                .get_mut("mcp")
                .and_then(|servers| servers.get_mut(MCP_SERVER_NAME))
                .and_then(|server| server.get_mut("environment"))
                .and_then(Value::as_object_mut)
            else {
                return Ok(());
            };
            environment.insert(
                "VIBE_INSPECTOR_ALLOWED_PROJECTS".to_string(),
                Value::String(project_ids.join(",")),
            );
            environment.insert(
                "VIBE_INSPECTOR_ALLOW_PROJECT_CREATE".to_string(),
                Value::String("1".to_string()),
            );
            let updated = serde_json::to_string_pretty(&document).map_err(storage_error)?;
            if parse_jsonc(&existing)? == document {
                return Ok(());
            }
            backup_existing(&target)?;
            write_private_file(&target, &updated)
        }
    }
}

fn home_dir() -> Result<PathBuf, AppError> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Storage("HOME is unavailable".to_string()))
}

fn kimi_code_home() -> Option<PathBuf> {
    std::env::var_os("KIMI_CODE_HOME").map(PathBuf::from)
}

fn xdg_config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
}

fn grok_home() -> Option<PathBuf> {
    std::env::var_os("GROK_HOME").map(PathBuf::from)
}

fn path_entries() -> Vec<PathBuf> {
    std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect())
        .unwrap_or_default()
}

pub fn detect_integrations() -> Vec<AgentIntegration> {
    let Ok(home) = home_dir() else {
        return Vec::new();
    };
    detect_agents(
        &home,
        &path_entries(),
        kimi_code_home().as_deref(),
        xdg_config_home().as_deref(),
        grok_home().as_deref(),
    )
}

pub fn install_integrations(
    project_ids: &[String],
    input: InstallAgentIntegrationsInput,
) -> Vec<AgentIntegrationResult> {
    let agent_ids = input.agent_ids.iter().take(10).cloned().collect::<Vec<_>>();
    let home = match home_dir() {
        Ok(home) => home,
        Err(error) => {
            return agent_ids
                .into_iter()
                .map(|id| {
                    AgentIntegrationResult::failed(
                        id.clone(),
                        AppError::Storage(format!("{id}: {error}")),
                    )
                })
                .collect()
        }
    };
    let command = mcp_binary_path().map(|path| path.display().to_string());
    let kimi_code_home = kimi_code_home();
    let xdg_config_home = xdg_config_home();
    let grok_home = grok_home();
    let all_paths = agent_paths(
        &home,
        kimi_code_home.as_deref(),
        xdg_config_home.as_deref(),
        grok_home.as_deref(),
    );
    let path_entries = path_entries();
    let detected_kinds = all_paths
        .iter()
        .filter(|paths| !detection_evidence(paths, &home, &path_entries).is_empty())
        .map(|paths| paths.kind)
        .collect::<Vec<_>>();
    agent_ids
        .into_iter()
        .map(|id| {
            let outcome = install_one(
                &id,
                &all_paths,
                &detected_kinds,
                &home,
                &command,
                project_ids,
            );
            match outcome {
                Ok(integration) => AgentIntegrationResult::installed(integration),
                Err(error) => AgentIntegrationResult::failed(id, error),
            }
        })
        .collect()
}

fn install_one(
    id: &str,
    all_paths: &[AgentPaths],
    detected_kinds: &[AgentKind],
    home: &Path,
    command: &Result<String, AppError>,
    project_ids: &[String],
) -> Result<AgentIntegration, AppError> {
    let kind = AgentKind::from_id(id)
        .ok_or_else(|| AppError::Validation(format!("unknown agent id: {id}")))?;
    if !detected_kinds.contains(&kind) {
        return Err(AppError::NotFound(format!(
            "{} is not installed on this Mac",
            kind.name()
        )));
    }
    let paths = all_paths
        .iter()
        .find(|paths| paths.kind == kind)
        .expect("agent_paths covers every kind");
    let command = command
        .as_ref()
        .map_err(|error| AppError::NotFound(format!("{id}: {error}")))?;
    install_agent(paths, home, command, project_ids)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "vibe-inspector-agents-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn env_fixture() -> BTreeMap<String, String> {
        mcp_env(
            "agent-codex",
            "a".repeat(64).as_str(),
            &["project-1".to_string()],
        )
    }

    fn find<'a>(integrations: &'a [AgentIntegration], id: &str) -> &'a AgentIntegration {
        integrations
            .iter()
            .find(|integration| integration.id == id)
            .expect("integration present")
    }

    #[test]
    fn detects_agents_from_config_directories() {
        let directory = TestDirectory::new("detect-dirs");
        let home = directory.path();
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        let integrations = detect_agents(home, &[], None, None, None);

        let codex = find(&integrations, "codex");
        assert!(codex.detected);
        assert_eq!(
            codex.detection_evidence,
            vec![home.join(".codex").display().to_string()]
        );
        assert_eq!(
            codex.skills_dir,
            home.join(".codex/skills").display().to_string()
        );
        assert_eq!(
            codex.mcp_config_path,
            home.join(".codex/config.toml").display().to_string()
        );
        assert!(!codex.mcp_configured);
        assert_eq!(codex.skill_states.len(), 5);
        assert!(codex
            .skill_states
            .iter()
            .all(|state| state.status == SkillInstallStatus::Missing));
        assert!(find(&integrations, "claude").detected);
        let kimi = find(&integrations, "kimi-code");
        assert!(!kimi.detected);
        assert!(kimi.detection_evidence.is_empty());
        assert_eq!(integrations.len(), 6);
    }

    #[test]
    fn detects_opencode_cursor_and_grok_global_layouts() {
        let directory = TestDirectory::new("detect-expanded");
        let home = directory.path();
        fs::create_dir_all(home.join(".config/opencode")).unwrap();
        fs::create_dir_all(home.join(".cursor")).unwrap();
        fs::create_dir_all(home.join(".grok")).unwrap();

        let integrations = detect_agents(home, &[], None, None, None);

        let opencode = find(&integrations, "opencode");
        assert!(opencode.detected);
        assert_eq!(
            opencode.skills_dir,
            home.join(".config/opencode/skills").display().to_string()
        );
        assert_eq!(
            opencode.mcp_config_path,
            home.join(".config/opencode/opencode.jsonc")
                .display()
                .to_string()
        );

        let cursor = find(&integrations, "cursor");
        assert!(cursor.detected);
        assert_eq!(
            cursor.mcp_config_path,
            home.join(".cursor/mcp.json").display().to_string()
        );

        let grok = find(&integrations, "grok");
        assert!(grok.detected);
        assert_eq!(
            grok.mcp_config_path,
            home.join(".grok/config.toml").display().to_string()
        );
    }

    #[test]
    fn detects_agents_from_path_binaries() {
        let directory = TestDirectory::new("detect-path");
        let home = directory.path().join("home");
        let bin = directory.path().join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("kimi"), "#!/bin/sh\n").unwrap();
        fs::set_permissions(bin.join("kimi"), fs::Permissions::from_mode(0o700)).unwrap();
        // A file with a matching name on PATH is not an installed CLI unless
        // the current user can execute it.
        fs::write(bin.join("codex"), "not executable\n").unwrap();

        let integrations = detect_agents(&home, std::slice::from_ref(&bin), None, None, None);

        let kimi = find(&integrations, "kimi-code");
        assert!(kimi.detected);
        assert_eq!(
            kimi.detection_evidence,
            vec![bin.join("kimi").display().to_string()]
        );
        assert!(!find(&integrations, "codex").detected);
        assert!(!find(&integrations, "claude").detected);
    }

    #[test]
    fn codex_skill_state_requires_openai_metadata() {
        let directory = TestDirectory::new("codex-skill-metadata");
        let home = directory.path().join("home");
        let codex_dir = home.join(".codex");
        let skill_dir = codex_dir.join("skills/inspector-init");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), SKILL_TEMPLATES[0].skill_md).unwrap();
        let paths = agent_paths(&home, None, None, None);
        let codex = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Codex)
            .unwrap();

        let state = skill_states(codex)
            .into_iter()
            .find(|state| state.id == "inspector-init")
            .unwrap();
        assert_eq!(state.status, SkillInstallStatus::Outdated);

        install_skills(codex).expect("repair Codex skill metadata");
        assert_eq!(
            fs::read_to_string(skill_dir.join("agents/openai.yaml")).unwrap(),
            SKILL_TEMPLATES[0].openai_yaml
        );
        assert!(skill_states(codex)
            .into_iter()
            .all(|state| state.status == SkillInstallStatus::Installed));
    }

    #[test]
    fn kimi_code_home_overrides_default_directory() {
        let directory = TestDirectory::new("kimi-home");
        let home = directory.path().join("home");
        let custom = directory.path().join("custom-kimi");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&custom).unwrap();

        let integrations = detect_agents(&home, &[], Some(&custom), None, None);

        let kimi = find(&integrations, "kimi-code");
        assert!(kimi.detected);
        assert_eq!(kimi.detection_evidence, vec![custom.display().to_string()]);
        assert_eq!(
            kimi.mcp_config_path,
            custom.join("mcp.json").display().to_string()
        );
        assert_eq!(kimi.skills_dir, custom.join("skills").display().to_string());
    }

    #[test]
    fn detects_cursor_from_user_applications_bundle() {
        let directory = TestDirectory::new("cursor-application");
        let home = directory.path().join("home");
        let application = home.join("Applications/Cursor.app");
        fs::create_dir_all(&application).unwrap();

        let integrations = detect_agents(&home, &[], None, None, None);

        let cursor = find(&integrations, "cursor");
        assert!(cursor.detected);
        assert!(cursor
            .detection_evidence
            .contains(&application.display().to_string()));
    }

    #[test]
    fn grok_home_overrides_default_directory() {
        let directory = TestDirectory::new("grok-home");
        let home = directory.path().join("home");
        let custom = directory.path().join("custom-grok");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&custom).unwrap();

        let integrations = detect_agents(&home, &[], None, None, Some(&custom));

        let grok = find(&integrations, "grok");
        assert!(grok.detected);
        assert_eq!(grok.detection_evidence, vec![custom.display().to_string()]);
        assert_eq!(
            grok.mcp_config_path,
            custom.join("config.toml").display().to_string()
        );
        assert_eq!(grok.skills_dir, custom.join("skills").display().to_string());
    }

    #[test]
    fn incomplete_mcp_entries_are_detected_and_repaired() {
        let directory = TestDirectory::new("incomplete-mcp");
        let home = directory.path().join("home");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        let paths = agent_paths(&home, None, None, None);
        let codex = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Codex)
            .unwrap();
        let claude = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Claude)
            .unwrap();

        let incomplete_toml =
            "[mcp_servers.vibe-inspector]\ncommand = \"/app/vibe-inspector-mcp\"\n";
        fs::write(&codex.mcp_config_path, incomplete_toml).unwrap();
        fs::write(
            &claude.mcp_config_path,
            r#"{"mcpServers":{"vibe-inspector":{"command":"/app/vibe-inspector-mcp"}}}"#,
        )
        .unwrap();
        assert!(!mcp_configured(codex));
        assert!(!mcp_configured(claude));

        let codex_env = mcp_env("agent-codex", &"a".repeat(64), &["project-1".to_string()]);
        let repaired_toml = upsert_codex_mcp_config(
            incomplete_toml,
            "/app/vibe-inspector-mcp",
            &codex_env,
            &["project-1".to_string()],
        )
        .unwrap();
        fs::write(&codex.mcp_config_path, repaired_toml).unwrap();

        let incomplete_json: Value =
            serde_json::from_str(&fs::read_to_string(&claude.mcp_config_path).unwrap()).unwrap();
        let repaired_json = merge_json_mcp_config(
            &incomplete_json,
            "/app/vibe-inspector-mcp",
            &mcp_env("agent-claude", &"b".repeat(64), &["project-1".to_string()]),
        )
        .unwrap();
        fs::write(
            &claude.mcp_config_path,
            serde_json::to_string_pretty(&repaired_json).unwrap(),
        )
        .unwrap();

        assert!(mcp_configured(codex));
        assert!(mcp_configured(claude));
    }

    #[test]
    fn json_merge_preserves_unknown_fields_and_is_idempotent() {
        let document = json!({
            "theme": "dark",
            "mcpServers": {
                "other": { "command": "/usr/bin/other", "args": ["--flag"] },
                "vibe-inspector": {
                    "command": "/old",
                    "custom": { "keep": true },
                    "env": {
                        "VIBE_INSPECTOR_SESSION_SECRET": "keep-secret",
                        "VIBE_INSPECTOR_SESSION_ID": "agent-other",
                        "CUSTOM_AGENT_SETTING": "keep"
                    }
                }
            }
        });
        let env = env_fixture();

        let merged = merge_json_mcp_config(&document, "/app/vibe-inspector-mcp", &env)
            .expect("merge config");

        assert_eq!(merged["theme"], "dark");
        assert_eq!(merged["mcpServers"]["other"]["args"][0], "--flag");
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["command"],
            "/app/vibe-inspector-mcp"
        );
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["env"]["VIBE_INSPECTOR_SESSION_ID"],
            "agent-codex"
        );
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["env"][SESSION_SECRET_KEY],
            "a".repeat(64)
        );
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["env"]["CUSTOM_AGENT_SETTING"],
            "keep"
        );
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["custom"]["keep"],
            true
        );
        assert_eq!(
            merged["mcpServers"][MCP_SERVER_NAME]["env"]["VIBE_INSPECTOR_ALLOW_PROJECT_CREATE"],
            "1"
        );

        let mut rotated_env = env.clone();
        rotated_env.insert(SESSION_SECRET_KEY.to_string(), "b".repeat(64));
        let merged_again = merge_json_mcp_config(&merged, "/app/vibe-inspector-mcp", &rotated_env)
            .expect("merge again");
        assert_eq!(merged, merged_again);

        assert!(merge_json_mcp_config(&json!(["not", "an", "object"]), "/bin/x", &env).is_err());
    }

    #[test]
    fn opencode_jsonc_merge_uses_local_server_shape() {
        let document = parse_jsonc(
            r#"{
                // Existing OpenCode settings remain intact.
                "theme": "system",
                "mcp": {
                    "other": { "type": "remote", "url": "https://example.test" },
                    "vibe-inspector": {
                        "type": "local",
                        "custom": "keep",
                        "environment": {
                            "VIBE_INSPECTOR_SESSION_SECRET": "keep-secret",
                            "CUSTOM_AGENT_SETTING": "keep"
                        }
                    }
                }
            }"#,
        )
        .expect("parse JSONC");
        let env = env_fixture();

        let merged = merge_opencode_mcp_config(&document, "/app/vibe-inspector-mcp", &env)
            .expect("merge OpenCode config");

        assert_eq!(merged["theme"], "system");
        assert_eq!(merged["mcp"]["other"]["type"], "remote");
        assert_eq!(merged["mcp"][MCP_SERVER_NAME]["type"], "local");
        assert_eq!(merged["mcp"][MCP_SERVER_NAME]["custom"], "keep");
        assert_eq!(
            merged["mcp"][MCP_SERVER_NAME]["command"][0],
            "/app/vibe-inspector-mcp"
        );
        assert_eq!(
            merged["mcp"][MCP_SERVER_NAME]["environment"][SESSION_SECRET_KEY],
            "a".repeat(64)
        );
        assert_eq!(
            merged["mcp"][MCP_SERVER_NAME]["environment"]["CUSTOM_AGENT_SETTING"],
            "keep"
        );

        let mut rotated_env = env.clone();
        rotated_env.insert(SESSION_SECRET_KEY.to_string(), "b".repeat(64));
        let merged_again =
            merge_opencode_mcp_config(&merged, "/app/vibe-inspector-mcp", &rotated_env).unwrap();
        assert_eq!(merged, merged_again);
    }

    #[test]
    fn codex_toml_upsert_is_idempotent() {
        let existing = "[mcp_servers.node_repl]\ncommand = \"node\"\n";
        let env = env_fixture();
        let projects = vec!["project-1".to_string()];

        let appended =
            upsert_codex_mcp_config(existing, "/app/vibe-inspector-mcp", &env, &projects)
                .expect("append block");
        assert!(appended.contains(CODEX_MCP_SECTION));
        assert!(appended.contains(CODEX_MCP_ENV_SECTION));
        assert!(appended.contains("command = \"/app/vibe-inspector-mcp\""));
        assert!(appended.contains("VIBE_INSPECTOR_SESSION_ID = \"agent-codex\""));
        assert!(appended.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS = \"project-1\""));
        assert!(appended.contains("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE = \"1\""));
        // Existing content is preserved.
        assert!(appended.contains("[mcp_servers.node_repl]"));

        let appended_again =
            upsert_codex_mcp_config(&appended, "/app/vibe-inspector-mcp", &env, &projects)
                .expect("update in place");
        assert_eq!(appended, appended_again);

        assert!(has_codex_mcp_section(&appended).unwrap());
        assert!(!has_codex_mcp_section("[mcp_servers.vibe-inspector-extra]\n").unwrap());
    }

    #[test]
    fn codex_section_detection_handles_toml_variants() {
        assert!(
            has_codex_mcp_section("[mcp_servers.\"vibe-inspector\"]\ncommand = \"x\"\n").unwrap()
        );
        assert!(
            has_codex_mcp_section("[ mcp_servers.vibe-inspector ]\ncommand = \"x\"\n").unwrap()
        );
        assert!(
            has_codex_mcp_section("[mcp_servers.vibe-inspector] # managed by VibeInspector\n")
                .unwrap()
        );
        assert!(
            has_codex_mcp_section("mcp_servers = { vibe-inspector = { command = \"x\" } }\n")
                .unwrap()
        );
        assert!(!has_codex_mcp_section("[mcp_servers.other]\ncommand = \"x\"\n").unwrap());

        // A corrupt file is an error, never a reason to append a duplicate.
        assert!(has_codex_mcp_section("command = [broken\n").is_err());
        assert!(
            upsert_codex_mcp_config("command = [broken\n", "/app/mcp", &env_fixture(), &[])
                .is_err()
        );

        let commented_header = "[mcp_servers.vibe-inspector] # existing\ncommand = \"/old\"\n";
        let updated = upsert_codex_mcp_config(
            commented_header,
            "/app/mcp",
            &env_fixture(),
            &["project-1".to_string()],
        )
        .expect("update a commented section header");
        assert!(updated.contains("[mcp_servers.vibe-inspector] # existing"));
        assert!(updated.contains("command = \"/app/mcp\""));
    }

    #[test]
    fn codex_existing_section_is_updated_in_place() {
        let kept_secret = "d".repeat(64);
        let existing = format!(
            concat!(
                "[mcp_servers.vibe-inspector]\n",
                "command = \"/old/path\"\n",
                "\n",
                "[mcp_servers.vibe-inspector.env]\n",
                "VIBE_INSPECTOR_SESSION_SECRET = \"{}\"\n",
                "VIBE_INSPECTOR_SESSION_ID = \"agent-other\"\n",
                "VIBE_INSPECTOR_ALLOWED_PROJECTS = \"pending-registration\"\n",
            ),
            kept_secret
        );
        let projects = vec!["project-1".to_string()];

        let updated = upsert_codex_mcp_config(&existing, "/new/path", &env_fixture(), &projects)
            .expect("update existing section");

        assert!(updated.contains("command = \"/new/path\""));
        assert!(!updated.contains("/old/path"));
        assert!(updated.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS = \"project-1\""));
        assert!(!updated.contains("pending-registration"));
        // The valid secret is preserved while the incorrect session ID is repaired.
        assert!(updated.contains(&format!(
            "VIBE_INSPECTOR_SESSION_SECRET = \"{kept_secret}\""
        )));
        assert!(!updated.contains("VIBE_INSPECTOR_SESSION_ID = \"agent-other\""));
        assert!(updated.contains("VIBE_INSPECTOR_SESSION_ID = \"agent-codex\""));
        // Agents installed before project auto-import gain the capability.
        assert!(updated.contains("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE = \"1\""));
        assert_eq!(updated.matches(CODEX_MCP_SECTION).count(), 1);

        // A second pass leaves the file unchanged.
        let repassed = upsert_codex_mcp_config(&updated, "/new/path", &env_fixture(), &projects)
            .expect("idempotent update");
        assert_eq!(updated, repassed);

        // An empty project list leaves the allow-list untouched.
        let kept = upsert_codex_mcp_config(&updated, "/new/path", &env_fixture(), &[])
            .expect("keep allow-list");
        assert!(kept.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS = \"project-1\""));
    }

    #[test]
    fn codex_env_key_matching_is_exact_and_inserts_missing() {
        let existing = concat!(
            "[mcp_servers.vibe-inspector]\n",
            "command = \"x\"\n",
            "\n",
            "[mcp_servers.vibe-inspector.env]\n",
            "VIBE_INSPECTOR_ALLOWED_PROJECTS_EXTRA = \"untouched\"\n",
        );

        let updated = update_codex_mcp_config(existing, None, Some("p1,p2")).expect("insert key");

        assert!(updated.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS_EXTRA = \"untouched\""));
        assert!(updated.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS = \"p1,p2\""));
        assert!(updated.contains("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE = \"1\""));
        // The missing key is inserted inside the env section.
        let env_at = updated.find(CODEX_MCP_ENV_SECTION).unwrap();
        let key_at = updated.find("VIBE_INSPECTOR_ALLOWED_PROJECTS = ").unwrap();
        let extra_at = updated
            .find("VIBE_INSPECTOR_ALLOWED_PROJECTS_EXTRA")
            .unwrap();
        assert!(env_at < key_at && key_at < extra_at);
    }

    #[test]
    fn writes_outside_containment_are_rejected() {
        let directory = TestDirectory::new("path-safety");
        let home = directory.path().join("home");
        let config_dir = home.join(".codex");
        fs::create_dir_all(&config_dir).unwrap();
        let outside = directory.path().join("outside.toml");

        let error = ensure_write_allowed(&outside, &config_dir)
            .expect_err("reject path outside the config directory");
        assert!(matches!(error, AppError::PathTraversal(_)));

        // A symlink inside the config directory pointing outside must fail.
        let link = config_dir.join("config.toml");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        fs::write(&outside, "secret = true\n").unwrap();
        let error = ensure_write_allowed(&link, &config_dir)
            .expect_err("reject symlink escaping the config directory");
        assert!(matches!(error, AppError::PathTraversal(_)));

        // A dangling symlink (target does not exist) must fail instead of
        // passing as a new file that a later write would follow outside.
        let dangling = config_dir.join("dangling.toml");
        std::os::unix::fs::symlink(directory.path().join("missing-target"), &dangling).unwrap();
        let error =
            ensure_write_allowed(&dangling, &config_dir).expect_err("reject dangling symlink");
        assert!(matches!(error, AppError::InvalidPath(_)));

        // A regular in-tree target passes.
        let allowed = ensure_write_allowed(&config_dir.join("new-file.toml"), &config_dir)
            .expect("allow in-tree write");
        assert!(allowed.ends_with("new-file.toml"));
    }

    #[test]
    fn install_rejects_config_and_skills_symlink_escapes() {
        let directory = TestDirectory::new("install-path-safety");
        let home = directory.path().join("home");
        let outside_config = directory.path().join("outside-config");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&outside_config).unwrap();
        std::os::unix::fs::symlink(&outside_config, home.join(".codex")).unwrap();

        let paths = agent_paths(&home, None, None, None);
        let codex = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Codex)
            .unwrap();
        let error = install_agent(codex, &home, "/app/vibe-inspector-mcp", &[])
            .expect_err("reject config directory symlink outside HOME");
        assert!(matches!(error, AppError::PathTraversal(_)));
        assert!(!outside_config
            .join("skills/inspector-init/SKILL.md")
            .exists());

        fs::remove_file(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".codex")).unwrap();
        let outside_skills = directory.path().join("outside-skills");
        fs::create_dir_all(&outside_skills).unwrap();
        std::os::unix::fs::symlink(&outside_skills, home.join(".codex/skills")).unwrap();

        let error = install_agent(codex, &home, "/app/vibe-inspector-mcp", &[])
            .expect_err("reject skills directory symlink outside agent config");
        assert!(matches!(error, AppError::PathTraversal(_)));
        assert!(!outside_skills.join("inspector-init/SKILL.md").exists());
    }

    #[test]
    fn backup_keeps_first_copy_and_is_private() {
        let directory = TestDirectory::new("backup");
        let config = directory.path().join("config.toml");
        fs::write(&config, "original = true\n").unwrap();
        fs::set_permissions(&config, fs::Permissions::from_mode(0o644)).unwrap();

        backup_existing(&config).expect("first backup");
        let backup = directory.path().join("config.toml.vibe-inspector-backup");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "original = true\n");
        assert_eq!(
            fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );

        // A second backup must not overwrite the pre-VibeInspector original.
        fs::write(&config, "changed = true\n").unwrap();
        backup_existing(&config).expect("second backup is skipped");
        assert_eq!(fs::read_to_string(&backup).unwrap(), "original = true\n");
    }

    #[test]
    fn mcp_binary_must_be_executable() {
        let directory = TestDirectory::new("mcp-executable");
        let helper = directory.path().join(MCP_BINARY_NAME);
        fs::write(&helper, "helper\n").unwrap();
        fs::set_permissions(&helper, fs::Permissions::from_mode(0o600)).unwrap();

        assert!(matches!(
            validate_mcp_binary(helper.clone()),
            Err(AppError::Validation(message)) if message.contains("not executable")
        ));

        fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            validate_mcp_binary(helper.clone()).unwrap(),
            helper.canonicalize().unwrap()
        );
    }

    #[test]
    fn install_one_rejects_an_undetected_agent() {
        let directory = TestDirectory::new("install-undetected");
        let home = directory.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let paths = agent_paths(&home, None, None, None);
        let command = Ok("/app/vibe-inspector-mcp".to_string());

        let error = install_one("codex", &paths, &[], &home, &command, &[])
            .expect_err("undetected target must be rejected");

        assert!(matches!(error, AppError::NotFound(message) if message.contains("Codex")));
        assert!(!home.join(".codex").exists());
    }

    #[test]
    fn kimi_code_home_outside_home_installs() {
        let directory = TestDirectory::new("kimi-outside");
        let home = directory.path().join("home");
        let custom = directory.path().join("kimi-outside-home");
        fs::create_dir_all(&home).unwrap();
        assert!(!custom.starts_with(&home));

        let paths = agent_paths(&home, Some(&custom), None, None);
        let kimi_paths = paths
            .iter()
            .find(|p| p.kind == AgentKind::KimiCode)
            .unwrap();
        let project_ids = vec!["project-1".to_string()];

        // The explicit KIMI_CODE_HOME override is a trusted location even
        // though it lives outside HOME.
        let result = install_agent(kimi_paths, &home, "/app/vibe-inspector-mcp", &project_ids)
            .expect("install kimi outside HOME");
        assert!(result.mcp_configured);
        assert!(custom.join("skills/inspector-init/SKILL.md").exists());
        let mcp_json = custom.join("mcp.json");
        assert_eq!(
            fs::metadata(&mcp_json).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let document: Value =
            serde_json::from_str(&fs::read_to_string(&mcp_json).unwrap()).unwrap();
        assert_eq!(
            document["mcpServers"][MCP_SERVER_NAME]["env"]["VIBE_INSPECTOR_SESSION_ID"],
            "agent-kimi-code"
        );
    }

    #[test]
    fn merged_project_ids_unions_registry_and_mcp_created_projects() {
        let registry_ids = vec!["registry-1".to_string(), "shared".to_string()];
        let stored = vec![
            StoredProject {
                id: "shared".to_string(),
                name: "Shared".to_string(),
                root_path: "/tmp/shared".to_string(),
                status: crate::storage::ProjectStatus::Ready,
                source: "mcp".to_string(),
                revision: 1,
                created_at: "1".to_string(),
                updated_at: "1".to_string(),
                archived_at: None,
            },
            StoredProject {
                id: "mcp-only".to_string(),
                name: "MCP".to_string(),
                root_path: "/tmp/mcp".to_string(),
                status: crate::storage::ProjectStatus::Ready,
                source: "mcp".to_string(),
                revision: 1,
                created_at: "1".to_string(),
                updated_at: "1".to_string(),
                archived_at: None,
            },
        ];

        let merged = merged_project_ids(&registry_ids, &stored);

        assert_eq!(merged, vec!["registry-1", "shared", "mcp-only"]);
    }

    #[test]
    fn allow_list_sync_updates_codex_and_json_configs() {
        let directory = TestDirectory::new("allow-list-sync");
        let home = directory.path().join("home");
        let codex_dir = home.join(".codex");
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&claude_dir).unwrap();

        let codex_config = codex_dir.join("config.toml");
        let env = mcp_env("agent-codex", &"b".repeat(64), &["old-project".to_string()]);
        // Simulate an install from before project auto-import existed.
        let legacy_codex = upsert_codex_mcp_config(
            "",
            "/app/vibe-inspector-mcp",
            &env,
            &["old-project".to_string()],
        )
        .unwrap()
        .replace("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE = \"1\"\n", "");
        assert!(!legacy_codex.contains("ALLOW_PROJECT_CREATE"));
        fs::write(&codex_config, &legacy_codex).unwrap();

        let claude_config = home.join(".claude.json");
        let mut document = merge_json_mcp_config(
            &json!({"theme": "dark"}),
            "/app/vibe-inspector-mcp",
            &mcp_env(
                "agent-claude",
                &"c".repeat(64),
                &["old-project".to_string()],
            ),
        )
        .unwrap();
        document["mcpServers"][MCP_SERVER_NAME]["env"]
            .as_object_mut()
            .unwrap()
            .remove("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE");
        fs::write(
            &claude_config,
            serde_json::to_string_pretty(&document).unwrap(),
        )
        .unwrap();

        let projects = vec!["project-a".to_string(), "project-b".to_string()];
        let paths = agent_paths(&home, None, None, None);
        let codex_paths = paths.iter().find(|p| p.kind == AgentKind::Codex).unwrap();
        let claude_paths = paths.iter().find(|p| p.kind == AgentKind::Claude).unwrap();

        sync_agent_allowlist(codex_paths, &home, &projects).expect("sync codex");
        sync_agent_allowlist(claude_paths, &home, &projects).expect("sync claude");

        let codex_text = fs::read_to_string(&codex_config).unwrap();
        assert!(codex_text.contains("VIBE_INSPECTOR_ALLOWED_PROJECTS = \"project-a,project-b\""));
        assert!(codex_text.contains("VIBE_INSPECTOR_ALLOW_PROJECT_CREATE = \"1\""));
        assert!(codex_text.contains(&"b".repeat(64)));
        assert!(codex_dir.join("config.toml.vibe-inspector-backup").exists());

        let claude_json: Value =
            serde_json::from_str(&fs::read_to_string(&claude_config).unwrap()).unwrap();
        assert_eq!(
            claude_json["mcpServers"][MCP_SERVER_NAME]["env"]["VIBE_INSPECTOR_ALLOWED_PROJECTS"],
            "project-a,project-b"
        );
        assert_eq!(
            claude_json["mcpServers"][MCP_SERVER_NAME]["env"]
                ["VIBE_INSPECTOR_ALLOW_PROJECT_CREATE"],
            "1"
        );
        assert_eq!(claude_json["theme"], "dark");
        assert_eq!(
            claude_json["mcpServers"][MCP_SERVER_NAME]["env"]["VIBE_INSPECTOR_SESSION_SECRET"],
            "c".repeat(64)
        );
        assert!(home.join(".claude.json.vibe-inspector-backup").exists());

        // Agents without a configured MCP server are left alone.
        let kimi_paths = paths
            .iter()
            .find(|p| p.kind == AgentKind::KimiCode)
            .unwrap();
        sync_agent_allowlist(kimi_paths, &home, &projects).expect("skip kimi");
        assert!(!home.join(".kimi-code/mcp.json").exists());
    }

    #[test]
    fn allow_list_sync_skips_empty_projects() {
        let directory = TestDirectory::new("allow-list-empty");
        let home = directory.path().join("home");
        let codex_dir = home.join(".codex");
        let claude_dir = home.join(".claude");
        fs::create_dir_all(&codex_dir).unwrap();
        fs::create_dir_all(&claude_dir).unwrap();

        let codex_config = codex_dir.join("config.toml");
        let env = mcp_env("agent-codex", &"b".repeat(64), &[]);
        let codex_text = upsert_codex_mcp_config("", "/app/vibe-inspector-mcp", &env, &[]).unwrap();
        assert!(codex_text.contains("pending-registration"));
        fs::write(&codex_config, &codex_text).unwrap();

        let claude_config = home.join(".claude.json");
        let document = merge_json_mcp_config(
            &json!({}),
            "/app/vibe-inspector-mcp",
            &mcp_env("agent-claude", &"c".repeat(64), &[]),
        )
        .unwrap();
        let claude_text = serde_json::to_string_pretty(&document).unwrap();
        fs::write(&claude_config, &claude_text).unwrap();

        // An empty registry must not clobber the existing allow-list; the
        // MCP server would refuse to start without one.
        let paths = agent_paths(&home, None, None, None);
        let codex_paths = paths.iter().find(|p| p.kind == AgentKind::Codex).unwrap();
        let claude_paths = paths.iter().find(|p| p.kind == AgentKind::Claude).unwrap();
        sync_agent_allowlist(codex_paths, &home, &[]).expect("skip empty codex sync");
        sync_agent_allowlist(claude_paths, &home, &[]).expect("skip empty claude sync");

        assert_eq!(fs::read_to_string(&codex_config).unwrap(), codex_text);
        assert_eq!(fs::read_to_string(&claude_config).unwrap(), claude_text);
        assert!(!codex_dir.join("config.toml.vibe-inspector-backup").exists());
        assert!(!home.join(".claude.json.vibe-inspector-backup").exists());
    }

    #[test]
    fn install_agent_writes_skills_and_private_configs() {
        let directory = TestDirectory::new("install");
        let home = directory.path().join("home");
        let codex_dir = home.join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();

        let paths = agent_paths(&home, None, None, None);
        let codex_paths = paths.iter().find(|p| p.kind == AgentKind::Codex).unwrap();
        let project_ids = vec!["project-1".to_string()];

        let result = install_agent(codex_paths, &home, "/app/vibe-inspector-mcp", &project_ids)
            .expect("install codex");
        assert!(result.detected);
        assert!(result.mcp_configured);
        assert!(result
            .skill_states
            .iter()
            .all(|state| state.status == SkillInstallStatus::Installed));

        let skill_md = codex_dir.join("skills/inspector-init/SKILL.md");
        assert_eq!(
            fs::read_to_string(&skill_md).unwrap(),
            SKILL_TEMPLATES[0].skill_md
        );
        assert!(codex_dir
            .join("skills/inspector-init/agents/openai.yaml")
            .exists());

        let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        assert!(config.contains(CODEX_MCP_SECTION));
        assert_eq!(
            fs::metadata(codex_dir.join("config.toml"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        // Re-installing does not duplicate the TOML section.
        install_agent(codex_paths, &home, "/app/vibe-inspector-mcp", &project_ids)
            .expect("reinstall codex");
        let config = fs::read_to_string(codex_dir.join("config.toml")).unwrap();
        assert_eq!(config.matches(CODEX_MCP_SECTION).count(), 1);

        // Claude skills omit agents/openai.yaml and a fresh .claude.json is 0600.
        let claude_paths = paths.iter().find(|p| p.kind == AgentKind::Claude).unwrap();
        install_agent(claude_paths, &home, "/app/vibe-inspector-mcp", &project_ids)
            .expect("install claude");
        assert!(claude_paths
            .skills_dir
            .join("inspector-init/SKILL.md")
            .exists());
        assert!(!claude_paths
            .skills_dir
            .join("inspector-init/agents/openai.yaml")
            .exists());
        let claude_config = home.join(".claude.json");
        assert_eq!(
            fs::metadata(&claude_config).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let first_claude_config = fs::read_to_string(&claude_config).unwrap();
        install_agent(claude_paths, &home, "/app/vibe-inspector-mcp", &project_ids)
            .expect("reinstall claude");
        assert_eq!(
            fs::read_to_string(&claude_config).unwrap(),
            first_claude_config
        );
        assert!(!home.join(".claude.json.vibe-inspector-backup").exists());
    }

    #[test]
    fn installs_opencode_cursor_and_grok_formats() {
        let directory = TestDirectory::new("install-expanded");
        let home = directory.path().join("home");
        fs::create_dir_all(home.join(".config/opencode")).unwrap();
        fs::create_dir_all(home.join(".cursor")).unwrap();
        fs::create_dir_all(home.join(".grok")).unwrap();
        fs::write(
            home.join(".config/opencode/opencode.jsonc"),
            "{\n  // keep this setting\n  \"theme\": \"system\"\n}\n",
        )
        .unwrap();
        fs::write(
            home.join(".grok/config.toml"),
            "[ui]\nscreen_mode = \"minimal\"\n",
        )
        .unwrap();

        let paths = agent_paths(&home, None, None, None);
        let projects = vec!["project-1".to_string()];

        let opencode = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::OpenCode)
            .unwrap();
        install_agent(opencode, &home, "/app/vibe-inspector-mcp", &projects)
            .expect("install OpenCode");
        let opencode_document =
            parse_jsonc(&fs::read_to_string(home.join(".config/opencode/opencode.jsonc")).unwrap())
                .unwrap();
        assert_eq!(opencode_document["theme"], "system");
        assert_eq!(opencode_document["mcp"][MCP_SERVER_NAME]["type"], "local");
        assert!(home
            .join(".config/opencode/opencode.jsonc.vibe-inspector-backup")
            .exists());

        let cursor = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Cursor)
            .unwrap();
        install_agent(cursor, &home, "/app/vibe-inspector-mcp", &projects).expect("install Cursor");
        let cursor_document: Value =
            serde_json::from_str(&fs::read_to_string(home.join(".cursor/mcp.json")).unwrap())
                .unwrap();
        assert_eq!(
            cursor_document["mcpServers"][MCP_SERVER_NAME]["type"],
            "stdio"
        );

        let grok = paths
            .iter()
            .find(|paths| paths.kind == AgentKind::Grok)
            .unwrap();
        install_agent(grok, &home, "/app/vibe-inspector-mcp", &projects).expect("install Grok");
        let grok_text = fs::read_to_string(home.join(".grok/config.toml")).unwrap();
        assert!(grok_text.contains("[ui]"));
        assert!(grok_text.contains(CODEX_MCP_SECTION));
        assert!(grok_text.contains("VIBE_INSPECTOR_SESSION_ID = \"agent-grok\""));

        for kind in [AgentKind::OpenCode, AgentKind::Cursor, AgentKind::Grok] {
            let target = paths.iter().find(|paths| paths.kind == kind).unwrap();
            assert!(target.skills_dir.join("inspector-init/SKILL.md").exists());
            assert!(!target
                .skills_dir
                .join("inspector-init/agents/openai.yaml")
                .exists());
        }
    }
}
