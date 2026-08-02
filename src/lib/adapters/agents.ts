import { invoke } from '@tauri-apps/api/core';
import { isTauriRuntime } from '$lib/adapters/tauri';

export type AgentSkillStatus = 'installed' | 'outdated' | 'missing';

export type AgentSkillState = {
  id: string;
  status: AgentSkillStatus;
};

export type AgentIntegration = {
  id: string;
  name: string;
  detected: boolean;
  detectionEvidence: string[];
  skillsDir: string;
  skillStates: AgentSkillState[];
  mcpConfigured: boolean;
  mcpConfigPath: string;
};

export type AgentSetupStatus = 'installed' | 'update-available' | 'not-installed' | 'not-detected';

export type AgentIntegrationError = {
  code: string;
  message: string;
};

export type AgentIntegrationResult = {
  agentId: string;
  integration?: AgentIntegration;
  error?: AgentIntegrationError;
};

export function agentSetupStatus(agent: AgentIntegration): AgentSetupStatus {
  if (!agent.detected) return 'not-detected';
  const skillsInstalled = agent.skillStates.filter((skill) => skill.status !== 'missing');
  if (skillsInstalled.length === 0 && !agent.mcpConfigured) return 'not-installed';
  const needsUpdate = agent.skillStates.some((skill) => skill.status !== 'installed');
  if (needsUpdate || !agent.mcpConfigured) return 'update-available';
  return 'installed';
}

const demoAgents: AgentIntegration[] = [
  {
    id: 'codex',
    name: 'Codex',
    detected: true,
    detectionEvidence: ['~/.codex', '~/.local/bin/codex'],
    skillsDir: '~/.codex/skills',
    skillStates: [
      { id: 'inspector-init', status: 'installed' },
      { id: 'inspector-sync', status: 'outdated' },
      { id: 'inspector-clarify', status: 'missing' },
      { id: 'inspector-review', status: 'missing' },
    ],
    mcpConfigured: false,
    mcpConfigPath: '~/.codex/config.toml',
  },
  {
    id: 'claude',
    name: 'Claude Code',
    detected: true,
    detectionEvidence: ['~/.claude'],
    skillsDir: '~/.claude/skills',
    skillStates: [
      { id: 'inspector-init', status: 'missing' },
      { id: 'inspector-sync', status: 'missing' },
      { id: 'inspector-clarify', status: 'missing' },
      { id: 'inspector-review', status: 'missing' },
    ],
    mcpConfigured: false,
    mcpConfigPath: '~/.claude.json',
  },
  {
    id: 'kimi-code',
    name: 'Kimi Code',
    detected: false,
    detectionEvidence: [],
    skillsDir: '~/.kimi-code/skills',
    skillStates: [],
    mcpConfigured: false,
    mcpConfigPath: '~/.kimi-code/mcp.json',
  },
];

export async function detectAgentIntegrations(): Promise<AgentIntegration[]> {
  if (!isTauriRuntime()) return structuredClone(demoAgents);
  return invoke<AgentIntegration[]>('detect_agent_integrations');
}

export async function installAgentIntegrations(
  agentIds: string[],
): Promise<AgentIntegrationResult[]> {
  if (!isTauriRuntime()) {
    return agentIds.map((agentId) => {
      const agent = demoAgents.find((entry) => entry.id === agentId);
      if (!agent || !agent.detected) {
        return { agentId, error: { code: 'not_found', message: 'Agent not detected' } };
      }
      return {
        agentId,
        integration: {
          ...structuredClone(agent),
          skillStates: agent.skillStates.map((skill) => ({ ...skill, status: 'installed' })),
          mcpConfigured: true,
        },
      };
    });
  }
  return invoke<AgentIntegrationResult[]>('install_agent_integrations', { input: { agentIds } });
}
