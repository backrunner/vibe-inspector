import { describe, expect, it } from 'vitest';
import type { SourceRef } from './entities';
import { buildSourceTree, sourceTreeFileCount } from './source-tree';

function source(id: string, path: string, startLine = 1, endLine = 3): SourceRef {
  return {
    id,
    path,
    startLine,
    endLine,
    symbol: '',
    label: path,
    status: 'current',
    contentHash: id,
  };
}

describe('buildSourceTree', () => {
  it('groups folders before files and sorts each group naturally', () => {
    const tree = buildSourceTree([
      source('root', 'README.md'),
      source('b', 'src/zeta.ts'),
      source('a', 'src/alpha.ts'),
      source('nested', 'packages/core/index.ts'),
    ]);

    expect(tree.map((node) => `${node.kind}:${node.name}`)).toEqual([
      'folder:packages',
      'folder:src',
      'file:README.md',
    ]);
    const src = tree[1];
    expect(src.kind === 'folder' ? src.children.map((node) => node.name) : []).toEqual([
      'alpha.ts',
      'zeta.ts',
    ]);
    expect(sourceTreeFileCount(tree)).toBe(4);
  });

  it('aggregates and orders multiple ranges for the same file', () => {
    const tree = buildSourceTree([
      source('later', 'src/index.ts', 40, 48),
      source('earlier', 'src/index.ts', 4, 12),
    ]);
    const src = tree[0];
    const file = src.kind === 'folder' ? src.children[0] : undefined;

    expect(file?.kind).toBe('file');
    expect(file?.kind === 'file' ? file.refs.map((ref) => ref.id) : []).toEqual([
      'earlier',
      'later',
    ]);
    expect(sourceTreeFileCount(tree)).toBe(1);
  });
});
