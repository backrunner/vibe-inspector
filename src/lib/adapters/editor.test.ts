import { describe, expect, it } from 'vitest';
import {
  isEditorWriteShortcut,
  languageForPath,
  lineNumberMinChars,
  readonlyEditorOptions,
  shortContentHash,
} from './editor';

describe('readonly editor adapter', () => {
  it('locks both Monaco and its backing DOM surface', () => {
    const options = readonlyEditorOptions({
      value: 'fn inspect() {}',
      path: 'src/inspect.rs',
      theme: 'vibe-inspector-dark',
      codeStartLine: 41,
    });

    expect(options).toMatchObject({
      readOnly: true,
      domReadOnly: true,
      contextmenu: false,
      quickSuggestions: false,
      suggestOnTriggerCharacters: false,
      minimap: { enabled: false },
      language: 'rust',
      wordWrap: 'off',
    });
    expect(typeof options.lineNumbers).toBe('function');
    expect((options.lineNumbers as (line: number) => string)(2)).toBe('42');
  });

  it('blocks save, paste, and cut shortcuts while preserving copy', () => {
    const shortcut = (key: string) => ({ key, metaKey: true, ctrlKey: false });
    expect(isEditorWriteShortcut(shortcut('s'))).toBe(true);
    expect(isEditorWriteShortcut(shortcut('V'))).toBe(true);
    expect(isEditorWriteShortcut(shortcut('x'))).toBe(true);
    expect(isEditorWriteShortcut(shortcut('c'))).toBe(false);
    expect(isEditorWriteShortcut({ key: 's', metaKey: false, ctrlKey: false })).toBe(false);
  });

  it('sizes the gutter from the displayed source line numbers', () => {
    expect(lineNumberMinChars('one\ntwo', 41)).toBe(3);
    expect(lineNumberMinChars('one\ntwo', 120_340)).toBe(6);
  });

  it('shortens long content hashes without changing short fixture hashes', () => {
    expect(shortContentHash('a93d2f1')).toBe('a93d2f1');
    expect(shortContentHash('0123456789abcdef'.repeat(4))).toBe('0123456789ab');
  });

  it.each([
    ['src/app.ts', 'typescript'],
    ['src/app.js', 'javascript'],
    ['package.json', 'json'],
    ['src/main.rs', 'rust'],
    ['src/routes/+page.svelte', 'html'],
    ['README.md', 'markdown'],
    ['scripts/index.py', 'python'],
    ['cmd/server/main.go', 'go'],
    ['Dockerfile', 'dockerfile'],
    ['infra/config.yaml', 'yaml'],
    ['unknown.asset', 'plaintext'],
  ] as const)('maps %s to %s syntax rendering', (path, language) => {
    expect(languageForPath(path)).toBe(language);
  });
});
