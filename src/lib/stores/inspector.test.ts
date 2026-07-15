import { describe, expect, it } from 'vitest';
import { findFirstFlow, findModule, flattenModules, seedProjects } from '../domain/entities';

describe('inspector domain fixtures', () => {
  it('flattens nested modules without losing their flow associations', () => {
    const project = seedProjects[0];
    const modules = flattenModules(project.modules);
    expect(modules.map((module) => module.id)).toContain('module-inspector-ui');
    expect(findModule(project, 'module-inspector-ui')?.flows).toHaveLength(1);
  });

  it('finds a first inspectable flow for a project', () => {
    const result = findFirstFlow(seedProjects[1]);
    expect(result?.module.id).toBe('module-checkout');
    expect(result?.flow.id).toBe('flow-checkout');
  });
});
