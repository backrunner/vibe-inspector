use std::io::{self, IsTerminal, Stdout, Write};

use clap::{Args, Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal, TerminalOptions, Viewport,
};
use serde::Serialize;
use serde_json::json;

use crate::{
    agent_integrations::{
        self, AgentIntegration, AgentIntegrationResult, InstallAgentIntegrationsInput,
    },
    error::AppError,
    storage::InspectorDatabase,
};

const CLI_COMMANDS: &[&str] = &["setup", "agents", "doctor"];
const SELECTOR_VIEWPORT_HEIGHT: u16 = 12;
const SELECTOR_MAX_WIDTH: u16 = 96;

#[derive(Debug, Parser)]
#[command(
    name = "vibe-inspector",
    version,
    about = "Install and inspect VibeInspector agent integrations"
)]
struct Cli {
    /// Emit a stable JSON envelope to stdout.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Select coding agents and install the global MCP server and skills.
    Setup(SetupArgs),
    /// Discover supported coding agents and inspect their installation state.
    Agents {
        #[command(subcommand)]
        command: AgentCommand,
    },
    /// Verify the MCP helper and global agent integration state.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum AgentCommand {
    /// List every supported agent, including targets not currently detected.
    List,
}

#[derive(Debug, Args)]
struct SetupArgs {
    /// Install into one agent ID. Repeat the flag or use comma-separated IDs.
    #[arg(long = "agent", value_delimiter = ',')]
    agent_ids: Vec<String>,

    /// Select every detected agent.
    #[arg(long, conflicts_with = "agent_ids")]
    all: bool,

    /// Skip the interactive selector and confirmation.
    #[arg(short = 'y', long)]
    yes: bool,

    /// Print the installation plan without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DoctorReport {
    version: &'static str,
    executable_path: String,
    mcp_binary_path: Option<String>,
    mcp_binary_error: Option<String>,
    supported_agents: usize,
    detected_agents: usize,
    configured_agents: usize,
    agents_needing_setup: usize,
    ready: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupPlanItem {
    agent_id: String,
    name: String,
    detected: bool,
    skills_dir: String,
    mcp_config_path: String,
    action: &'static str,
}

pub fn run_if_requested() -> Option<i32> {
    let arguments = std::env::args().collect::<Vec<_>>();
    if !is_cli_invocation(&arguments) {
        return None;
    }
    let json_output = arguments.iter().any(|argument| argument == "--json");
    let command_name = requested_command_name(&arguments);
    Some(match Cli::try_parse_from(arguments) {
        Ok(cli) => run(cli),
        Err(error) => {
            let exit_code = error.exit_code();
            if json_output && exit_code != 0 {
                print_json(usage_error_value(command_name, &error));
            } else {
                let _ = error.print();
            }
            exit_code
        }
    })
}

fn usage_error_value(command_name: &str, error: &clap::Error) -> serde_json::Value {
    let message = error.to_string();
    json!({
        "ok": false,
        "command": command_name,
        "error": {
            "code": "usage",
            "message": message.trim(),
        },
    })
}

fn requested_command_name(arguments: &[String]) -> &'static str {
    if arguments.iter().any(|argument| argument == "setup") {
        "setup"
    } else if arguments.iter().any(|argument| argument == "agents") {
        "agents.list"
    } else if arguments.iter().any(|argument| argument == "doctor") {
        "doctor"
    } else {
        "cli"
    }
}

fn is_cli_invocation(arguments: &[String]) -> bool {
    arguments.iter().skip(1).any(|argument| {
        CLI_COMMANDS.contains(&argument.as_str())
            || matches!(
                argument.as_str(),
                "--json" | "--help" | "-h" | "--version" | "-V"
            )
    })
}

fn run(cli: Cli) -> i32 {
    let command_name = match &cli.command {
        Command::Setup(_) => "setup",
        Command::Agents { .. } => "agents.list",
        Command::Doctor => "doctor",
    };
    let result = match cli.command {
        Command::Setup(arguments) => run_setup(arguments, cli.json),
        Command::Agents {
            command: AgentCommand::List,
        } => run_agents_list(cli.json).map(|_| 0),
        Command::Doctor => run_doctor(cli.json).map(|_| 0),
    };
    match result {
        Ok(exit_code) => exit_code,
        Err(error) => {
            if cli.json {
                print_json(json!({
                    "ok": false,
                    "command": command_name,
                    "error": error,
                }));
            } else {
                eprintln!("vibe-inspector: {error}");
            }
            1
        }
    }
}

fn run_agents_list(json_output: bool) -> Result<(), AppError> {
    let integrations = agent_integrations::detect_integrations();
    if json_output {
        print_json(json!({
            "ok": true,
            "command": "agents.list",
            "data": { "agents": integrations },
        }));
        return Ok(());
    }

    println!("VibeInspector agent integrations\n");
    for agent in integrations {
        let detection = if agent.detected {
            "detected"
        } else {
            "not detected"
        };
        println!(
            "{:<14} {:<13} {}",
            agent.id,
            detection,
            setup_status(&agent)
        );
        println!("  skills  {}", agent.skills_dir);
        println!("  MCP     {}", agent.mcp_config_path);
    }
    Ok(())
}

fn run_doctor(json_output: bool) -> Result<(), AppError> {
    let integrations = agent_integrations::detect_integrations();
    let mcp_binary = agent_integrations::mcp_binary_path();
    let report = DoctorReport {
        version: env!("CARGO_PKG_VERSION"),
        executable_path: std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "unknown".to_string()),
        mcp_binary_path: mcp_binary
            .as_ref()
            .ok()
            .map(|path| path.display().to_string()),
        mcp_binary_error: mcp_binary.as_ref().err().map(ToString::to_string),
        supported_agents: integrations.len(),
        detected_agents: integrations.iter().filter(|agent| agent.detected).count(),
        configured_agents: integrations
            .iter()
            .filter(|agent| agent.detected && !agent.needs_install())
            .count(),
        agents_needing_setup: integrations
            .iter()
            .filter(|agent| agent.detected && agent.needs_install())
            .count(),
        ready: mcp_binary.is_ok(),
    };

    if json_output {
        print_json(json!({
            "ok": true,
            "command": "doctor",
            "data": report,
        }));
    } else {
        println!("VibeInspector {}", report.version);
        println!("CLI       {}", report.executable_path);
        println!(
            "MCP       {}",
            report
                .mcp_binary_path
                .as_deref()
                .unwrap_or("missing; install vibe-inspector-mcp beside this binary")
        );
        println!(
            "Agents    {} detected / {} supported",
            report.detected_agents, report.supported_agents
        );
        println!("Ready     {}", if report.ready { "yes" } else { "no" });
        if let Some(error) = report.mcp_binary_error {
            println!("Problem   {error}");
        }
    }
    Ok(())
}

fn run_setup(arguments: SetupArgs, json_output: bool) -> Result<i32, AppError> {
    let integrations = agent_integrations::detect_integrations();
    let mut selected = resolve_selection(&integrations, &arguments)?;
    let can_interact = io::stdin().is_terminal() && io::stdout().is_terminal();

    if should_run_selector(&arguments, json_output, can_interact)? {
        selected = run_selector(&integrations, &selected)?;
    }

    if selected.is_empty() {
        return Err(AppError::AgentSetup(
            "no detected agents were selected".to_string(),
        ));
    }

    let plan = selected
        .iter()
        .filter_map(|id| integrations.iter().find(|agent| &agent.id == id))
        .map(|agent| SetupPlanItem {
            agent_id: agent.id.clone(),
            name: agent.name.clone(),
            detected: agent.detected,
            skills_dir: agent.skills_dir.clone(),
            mcp_config_path: agent.mcp_config_path.clone(),
            action: if agent.needs_install() {
                "install-or-update"
            } else {
                "refresh"
            },
        })
        .collect::<Vec<_>>();

    let mcp_binary = agent_integrations::mcp_binary_path()?;
    if arguments.dry_run {
        if json_output {
            print_json(json!({
                "ok": true,
                "command": "setup",
                "data": {
                    "dryRun": true,
                    "mcpBinaryPath": mcp_binary,
                    "agents": plan,
                },
            }));
        } else {
            println!("VibeInspector setup plan\n");
            for item in plan {
                println!("{} ({})", item.name, item.agent_id);
                println!("  skills  {}", item.skills_dir);
                println!("  MCP     {}", item.mcp_config_path);
            }
            println!("\nNo files were changed (--dry-run).");
        }
        return Ok(0);
    }

    let project_ids = active_project_ids();
    let results = agent_integrations::install_integrations(
        &project_ids,
        InstallAgentIntegrationsInput {
            agent_ids: selected,
        },
    );
    let failed = results
        .iter()
        .filter(|result| result.error.is_some())
        .count();

    if failed > 0 {
        if json_output {
            print_json(json!({
                "ok": false,
                "command": "setup",
                "data": {
                    "dryRun": false,
                    "mcpBinaryPath": mcp_binary,
                    "results": results,
                },
                "error": {
                    "code": "partial_failure",
                    "message": format!("{failed} agent installation(s) failed"),
                },
            }));
        } else {
            print_setup_results(&results);
        }
        return Ok(1);
    }

    if json_output {
        print_json(json!({
            "ok": true,
            "command": "setup",
            "data": {
                "dryRun": false,
                "mcpBinaryPath": mcp_binary,
                "results": results,
            },
        }));
    } else {
        print_setup_results(&results);
    }
    Ok(0)
}

fn should_run_selector(
    arguments: &SetupArgs,
    json_output: bool,
    can_interact: bool,
) -> Result<bool, AppError> {
    if arguments.yes || arguments.dry_run {
        return Ok(false);
    }
    if json_output || !can_interact {
        return Err(AppError::AgentSetup(
            "setup requires confirmation; attach a terminal or pass --yes or --dry-run".to_string(),
        ));
    }
    Ok(true)
}

fn resolve_selection(
    integrations: &[AgentIntegration],
    arguments: &SetupArgs,
) -> Result<Vec<String>, AppError> {
    if !arguments.agent_ids.is_empty() {
        let mut selected = Vec::new();
        for requested in &arguments.agent_ids {
            let requested = if requested == "claude-code" {
                "claude"
            } else {
                requested
            };
            let agent = integrations
                .iter()
                .find(|agent| agent.id == requested)
                .ok_or_else(|| AppError::Validation(format!("unknown agent id: {requested}")))?;
            if !agent.detected {
                return Err(AppError::NotFound(format!(
                    "{} is not installed on this Mac",
                    agent.name
                )));
            }
            if !selected.contains(&agent.id) {
                selected.push(agent.id.clone());
            }
        }
        return Ok(selected);
    }

    Ok(integrations
        .iter()
        .filter(|agent| agent.detected && (arguments.all || agent.needs_install()))
        .map(|agent| agent.id.clone())
        .collect())
}

fn active_project_ids() -> Vec<String> {
    let Ok(database) = InspectorDatabase::for_macos_user() else {
        return Vec::new();
    };
    if database.initialize().is_err() {
        return Vec::new();
    }
    database
        .list_projects(false)
        .map(|projects| projects.into_iter().map(|project| project.id).collect())
        .unwrap_or_default()
}

fn setup_status(agent: &AgentIntegration) -> &'static str {
    if !agent.detected {
        "not detected"
    } else if !agent.needs_install() {
        "installed"
    } else if agent.mcp_configured
        || agent
            .skill_states
            .iter()
            .any(|skill| skill.status != agent_integrations::SkillInstallStatus::Missing)
    {
        "update available"
    } else {
        "not installed"
    }
}

fn print_setup_results(results: &[AgentIntegrationResult]) {
    println!("VibeInspector setup\n");
    for result in results {
        if let Some(integration) = &result.integration {
            println!("[ok] {}", integration.name);
            println!("     skills  {}", integration.skills_dir);
            println!("     MCP     {}", integration.mcp_config_path);
        } else if let Some(error) = &result.error {
            println!("[failed] {}: {error}", result.agent_id);
        }
    }
}

fn print_json(value: serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(&value).expect("CLI response is serializable")
    );
}

struct SelectorState {
    cursor: usize,
    selected: Vec<String>,
}

fn run_selector(
    integrations: &[AgentIntegration],
    initial_selection: &[String],
) -> Result<Vec<String>, AppError> {
    let mut terminal = TerminalSession::start().map_err(io_error)?;
    let mut state = SelectorState {
        cursor: integrations
            .iter()
            .position(|agent| agent.detected)
            .unwrap_or(0),
        selected: initial_selection.to_vec(),
    };

    loop {
        terminal
            .terminal
            .draw(|frame| render_selector(frame, integrations, &state))
            .map_err(io_error)?;
        let Event::Key(key) = event::read().map_err(io_error)? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => move_cursor(&mut state, integrations, -1),
            KeyCode::Down | KeyCode::Char('j') => move_cursor(&mut state, integrations, 1),
            KeyCode::Char(' ') => toggle_current(&mut state, integrations),
            KeyCode::Char('a') => toggle_all(&mut state, integrations),
            KeyCode::Enter if !state.selected.is_empty() => return Ok(state.selected),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Err(AppError::AgentSetup("setup cancelled".to_string()))
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                return Err(AppError::AgentSetup("setup cancelled".to_string()))
            }
            _ => {}
        }
    }
}

fn move_cursor(state: &mut SelectorState, integrations: &[AgentIntegration], delta: isize) {
    if integrations.is_empty() {
        return;
    }
    for _ in 0..integrations.len() {
        state.cursor =
            (state.cursor as isize + delta).rem_euclid(integrations.len() as isize) as usize;
        if integrations[state.cursor].detected {
            break;
        }
    }
}

fn toggle_current(state: &mut SelectorState, integrations: &[AgentIntegration]) {
    let Some(agent) = integrations.get(state.cursor) else {
        return;
    };
    if !agent.detected {
        return;
    }
    if let Some(index) = state.selected.iter().position(|id| id == &agent.id) {
        state.selected.remove(index);
    } else {
        state.selected.push(agent.id.clone());
    }
}

fn toggle_all(state: &mut SelectorState, integrations: &[AgentIntegration]) {
    let detected = integrations
        .iter()
        .filter(|agent| agent.detected)
        .map(|agent| agent.id.clone())
        .collect::<Vec<_>>();
    if detected.iter().all(|id| state.selected.contains(id)) {
        state.selected.clear();
    } else {
        state.selected = detected;
    }
}

fn render_selector(
    frame: &mut Frame<'_>,
    integrations: &[AgentIntegration],
    state: &SelectorState,
) {
    let frame_area = frame.area();
    let area = Rect {
        width: frame_area.width.min(SELECTOR_MAX_WIDTH),
        ..frame_area
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" VibeInspector setup ")
        .border_style(Style::default().fg(Color::Rgb(82, 82, 91)))
        .style(Style::default().bg(Color::Rgb(24, 24, 27)));
    let inner = block.inner(area).inner(ratatui::layout::Margin::new(1, 0));
    frame.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(inner);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Choose the detected agents to configure.",
            Style::default().fg(Color::Rgb(244, 244, 245)),
        )),
        Line::from(Span::styled(
            format!(
                "{} detected | {} selected",
                integrations.iter().filter(|agent| agent.detected).count(),
                state.selected.len()
            ),
            Style::default().fg(Color::Rgb(161, 161, 170)),
        )),
    ]);
    frame.render_widget(header, chunks[0]);

    let items = integrations
        .iter()
        .enumerate()
        .map(|(index, agent)| {
            let checked = state.selected.contains(&agent.id);
            let checkbox = if checked { "[x]" } else { "[ ]" };
            let selected = index == state.cursor;
            let base = if agent.detected {
                Color::Rgb(244, 244, 245)
            } else {
                Color::Rgb(113, 113, 122)
            };
            let marker = if selected { ">" } else { " " };
            let checkbox = if agent.detected { checkbox } else { "[-]" };
            let status_color = match setup_status(agent) {
                "installed" => Color::Rgb(74, 222, 128),
                "update available" => Color::Rgb(251, 191, 36),
                _ => Color::Rgb(161, 161, 170),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} {checkbox} "), Style::default().fg(base)),
                Span::styled(
                    format!("{:<14}", agent.name),
                    Style::default().fg(base).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<18}", setup_status(agent)),
                    Style::default().fg(status_color),
                ),
                Span::styled(
                    compact_path(&agent.mcp_config_path),
                    Style::default().fg(Color::Rgb(113, 113, 122)),
                ),
            ]))
            .style(if selected {
                Style::default().bg(Color::Rgb(39, 39, 42))
            } else {
                Style::default()
            })
        })
        .collect::<Vec<_>>();
    let mut list_state = ListState::default();
    list_state.select(Some(state.cursor));
    frame.render_stateful_widget(
        List::new(items).highlight_style(Style::default()),
        chunks[1],
        &mut list_state,
    );

    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("Space", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" toggle   "),
            Span::styled("a", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" all   "),
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" install   "),
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]),
        Line::from(Span::styled(
            "Installs global MCP configuration and inspector skills.",
            Style::default().fg(Color::Rgb(113, 113, 122)),
        )),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(footer, chunks[2]);
}

fn compact_path(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return path.to_string();
    };
    let home = home.to_string_lossy();
    path.strip_prefix(home.as_ref())
        .filter(|suffix| suffix.is_empty() || suffix.starts_with('/'))
        .map(|suffix| format!("~{suffix}"))
        .unwrap_or_else(|| path.to_string())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn start() -> io::Result<Self> {
        enable_raw_mode()?;
        let backend = CrosstermBackend::new(io::stdout());
        match Terminal::with_options(
            backend,
            TerminalOptions {
                viewport: Viewport::Inline(SELECTOR_VIEWPORT_HEIGHT),
            },
        ) {
            Ok(terminal) => Ok(Self { terminal }),
            Err(error) => {
                let _ = disable_raw_mode();
                Err(error)
            }
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
        let _ = writeln!(io::stdout());
    }
}

fn io_error(error: impl std::fmt::Display) -> AppError {
    AppError::Storage(format!("terminal UI failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn integration(id: &str, detected: bool, configured: bool) -> AgentIntegration {
        AgentIntegration {
            id: id.to_string(),
            name: id.to_string(),
            detected,
            detection_evidence: Vec::new(),
            skills_dir: format!("/tmp/{id}/skills"),
            skill_states: Vec::new(),
            mcp_configured: configured,
            mcp_config_path: format!("/tmp/{id}/mcp.json"),
        }
    }

    #[test]
    fn recognizes_only_explicit_cli_invocations() {
        assert!(!is_cli_invocation(&["vibe-inspector".to_string()]));
        assert!(is_cli_invocation(&[
            "vibe-inspector".to_string(),
            "setup".to_string()
        ]));
        assert!(is_cli_invocation(&[
            "vibe-inspector".to_string(),
            "--json".to_string(),
            "doctor".to_string()
        ]));
        assert!(is_cli_invocation(&[
            "vibe-inspector".to_string(),
            "--json".to_string()
        ]));
        assert_eq!(
            requested_command_name(&[
                "vibe-inspector".to_string(),
                "--json".to_string(),
                "setup".to_string(),
                "--unknown".to_string(),
            ]),
            "setup"
        );
    }

    #[test]
    fn default_selection_includes_only_detected_agents_needing_setup() {
        let agents = vec![
            integration("codex", true, false),
            integration("claude", true, true),
            integration("cursor", false, false),
        ];
        let arguments = SetupArgs {
            agent_ids: Vec::new(),
            all: false,
            yes: false,
            dry_run: false,
        };

        assert_eq!(
            resolve_selection(&agents, &arguments).unwrap(),
            vec!["codex"]
        );
    }

    #[test]
    fn explicit_selection_accepts_claude_code_alias_and_rejects_missing_agent() {
        let agents = vec![integration("claude", true, false)];
        let arguments = SetupArgs {
            agent_ids: vec!["claude-code".to_string()],
            all: false,
            yes: true,
            dry_run: false,
        };
        assert_eq!(
            resolve_selection(&agents, &arguments).unwrap(),
            vec!["claude"]
        );

        let arguments = SetupArgs {
            agent_ids: vec!["cursor".to_string()],
            all: false,
            yes: true,
            dry_run: false,
        };
        assert!(resolve_selection(&agents, &arguments).is_err());
    }

    #[test]
    fn every_live_write_requires_tui_or_yes() {
        let interactive_all = SetupArgs {
            agent_ids: Vec::new(),
            all: true,
            yes: false,
            dry_run: false,
        };
        assert!(should_run_selector(&interactive_all, false, true).unwrap());
        assert!(should_run_selector(&interactive_all, true, true).is_err());
        assert!(should_run_selector(&interactive_all, false, false).is_err());

        let confirmed = SetupArgs {
            yes: true,
            ..interactive_all
        };
        assert!(!should_run_selector(&confirmed, true, false).unwrap());
    }

    #[test]
    fn clap_failures_use_the_json_error_envelope() {
        let arguments = ["vibe-inspector", "--json", "setup", "--unknown-option"];
        let error = Cli::try_parse_from(arguments).expect_err("invalid option");

        let value = usage_error_value("setup", &error);

        assert_eq!(value["ok"], false);
        assert_eq!(value["command"], "setup");
        assert_eq!(value["error"]["code"], "usage");
        assert!(value["error"]["message"]
            .as_str()
            .unwrap()
            .contains("--unknown-option"));
    }
}
