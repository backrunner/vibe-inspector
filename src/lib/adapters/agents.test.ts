import { describe, expect, it } from 'vitest';
import {
  agentSetupStatus,
  detectAgentIntegrations,
  installAgentIntegrations,
  type AgentIntegration,
} from './agents';

function agent(overrides: Partial<AgentIntegration>): AgentIntegration {
  return {
    id: 'codex',
    name: 'Codex',
    detected: true,
    detectionEvidence: ['~/.codex'],
    skillsDir: '~/.codex/skills',
    skillStates: [
      { id: 'inspector-init', status: 'installed' },
      { id: 'inspector-sync', status: 'installed' },
      { id: 'inspector-clarify', status: 'installed' },
    ],
    mcpConfigured: true,
    mcpConfigPath: '~/.codex/config.toml',
    ...overrides,
  };
}

describe('agentSetupStatus', () => {
  it('reports not-detected agents before anything else', () => {
    expect(agentSetupStatus(agent({ detected: false }))).toBe('not-detected');
  });

  it('reports installed only when every skill and MCP are configured', () => {
    expect(agentSetupStatus(agent({}))).toBe('installed');
    expect(agentSetupStatus(agent({ mcpConfigured: false }))).toBe('update-available');
  });

  it('reports update-available when any skill is outdated or missing', () => {
    expect(
      agentSetupStatus(
        agent({
          skillStates: [
            { id: 'inspector-init', status: 'installed' },
            { id: 'inspector-sync', status: 'outdated' },
          ],
        }),
      ),
    ).toBe('update-available');
  });

  it('reports not-installed when nothing has been deployed yet', () => {
    expect(
      agentSetupStatus(
        agent({
          skillStates: [
            { id: 'inspector-init', status: 'missing' },
            { id: 'inspector-sync', status: 'missing' },
          ],
          mcpConfigured: false,
        }),
      ),
    ).toBe('not-installed');
  });
});

describe('browser preview adapters', () => {
  it('detects demo agents outside the Tauri runtime', async () => {
    const agents = await detectAgentIntegrations();
    expect(agents.length).toBeGreaterThan(0);
    expect(agents.some((entry) => entry.detected)).toBe(true);
  });

  it('marks selected demo agents as installed without touching others', async () => {
    const results = await installAgentIntegrations(['claude', 'kimi-code']);
    const claude = results.find((entry) => entry.agentId === 'claude');
    const kimi = results.find((entry) => entry.agentId === 'kimi-code');
    expect(claude?.integration && agentSetupStatus(claude.integration)).toBe('installed');
    expect(kimi?.error?.code).toBe('not_found');
  });
});
