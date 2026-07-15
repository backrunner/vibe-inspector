import type { editor } from 'monaco-editor/esm/vs/editor/editor.api';

export type EditorLanguage =
  | 'cpp'
  | 'csharp'
  | 'css'
  | 'dart'
  | 'dockerfile'
  | 'go'
  | 'graphql'
  | 'html'
  | 'ini'
  | 'java'
  | 'javascript'
  | 'json'
  | 'kotlin'
  | 'less'
  | 'lua'
  | 'markdown'
  | 'php'
  | 'plaintext'
  | 'protobuf'
  | 'python'
  | 'ruby'
  | 'rust'
  | 'scss'
  | 'shell'
  | 'sql'
  | 'swift'
  | 'typescript'
  | 'xml'
  | 'yaml';

export type ReadonlyEditorOptionsInput = {
  value: string;
  path: string;
  theme: string;
  codeStartLine: number;
};

export function readonlyEditorOptions({
  value,
  path,
  theme,
  codeStartLine,
}: ReadonlyEditorOptionsInput): editor.IStandaloneEditorConstructionOptions {
  return {
    value,
    language: languageForPath(path),
    theme,
    readOnly: true,
    domReadOnly: true,
    contextmenu: false,
    quickSuggestions: false,
    suggestOnTriggerCharacters: false,
    minimap: { enabled: false },
    lineNumbers: (lineNumber) => String(lineNumber + codeStartLine - 1),
    lineNumbersMinChars: lineNumberMinChars(value, codeStartLine),
    glyphMargin: false,
    folding: true,
    renderLineHighlight: 'all',
    wordWrap: 'off',
    scrollBeyondLastLine: false,
    automaticLayout: true,
    padding: { top: 12, bottom: 18 },
    fontSize: 13,
    lineHeight: 20,
    fontFamily: 'SFMono-Regular, SF Mono, JetBrains Mono, ui-monospace, monospace',
    fontLigatures: false,
    fontWeight: '400',
    letterSpacing: 0,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
  };
}

export function lineNumberMinChars(value: string, codeStartLine: number): number {
  const lineCount = value.split('\n').length;
  const finalLine = Math.max(1, codeStartLine) + lineCount - 1;
  return Math.max(3, String(finalLine).length);
}

export function shortContentHash(hash: string): string {
  return hash.length > 12 ? hash.slice(0, 12) : hash;
}

export function languageForPath(path: string): EditorLanguage {
  const normalizedPath = path.toLowerCase();
  const fileName = normalizedPath.split('/').at(-1) ?? normalizedPath;
  if (fileName === 'dockerfile' || fileName.startsWith('dockerfile.')) return 'dockerfile';

  const extension = fileName.includes('.') ? fileName.split('.').at(-1) : '';
  if (extension === 'ts' || extension === 'tsx') return 'typescript';
  if (['js', 'jsx', 'mjs', 'cjs'].includes(extension ?? '')) return 'javascript';
  if (extension === 'json' || extension === 'jsonc') return 'json';
  if (extension === 'rs') return 'rust';
  if (['html', 'htm', 'svelte', 'vue'].includes(extension ?? '')) return 'html';
  if (extension === 'css') return 'css';
  if (extension === 'scss') return 'scss';
  if (extension === 'less') return 'less';
  if (extension === 'md' || extension === 'markdown') return 'markdown';
  if (extension === 'py' || extension === 'pyw') return 'python';
  if (extension === 'go') return 'go';
  if (extension === 'java') return 'java';
  if (extension === 'kt' || extension === 'kts') return 'kotlin';
  if (['c', 'h', 'cc', 'cpp', 'cxx', 'hpp'].includes(extension ?? '')) return 'cpp';
  if (extension === 'cs') return 'csharp';
  if (['sh', 'bash', 'zsh', 'fish'].includes(extension ?? '')) return 'shell';
  if (extension === 'yaml' || extension === 'yml') return 'yaml';
  if (['toml', 'ini', 'conf'].includes(extension ?? '')) return 'ini';
  if (extension === 'sql') return 'sql';
  if (extension === 'swift') return 'swift';
  if (extension === 'dart') return 'dart';
  if (extension === 'php') return 'php';
  if (extension === 'rb') return 'ruby';
  if (extension === 'lua') return 'lua';
  if (extension === 'graphql' || extension === 'gql') return 'graphql';
  if (extension === 'proto') return 'protobuf';
  if (extension === 'xml' || extension === 'svg') return 'xml';
  return 'plaintext';
}

export async function loadEditorLanguage(path: string): Promise<EditorLanguage> {
  const language = languageForPath(path);
  await languageLoaders[language]?.();
  return language;
}

const languageLoaders: Partial<Record<EditorLanguage, () => Promise<unknown>>> = {
  cpp: () => import('monaco-editor/esm/vs/basic-languages/cpp/cpp.contribution'),
  csharp: () => import('monaco-editor/esm/vs/basic-languages/csharp/csharp.contribution'),
  css: () => import('monaco-editor/esm/vs/basic-languages/css/css.contribution'),
  dart: () => import('monaco-editor/esm/vs/basic-languages/dart/dart.contribution'),
  dockerfile: () =>
    import('monaco-editor/esm/vs/basic-languages/dockerfile/dockerfile.contribution'),
  go: () => import('monaco-editor/esm/vs/basic-languages/go/go.contribution'),
  graphql: () => import('monaco-editor/esm/vs/basic-languages/graphql/graphql.contribution'),
  html: () => import('monaco-editor/esm/vs/basic-languages/html/html.contribution'),
  ini: () => import('monaco-editor/esm/vs/basic-languages/ini/ini.contribution'),
  java: () => import('monaco-editor/esm/vs/basic-languages/java/java.contribution'),
  javascript: () =>
    import('monaco-editor/esm/vs/basic-languages/javascript/javascript.contribution'),
  json: () => import('monaco-editor/esm/vs/language/json/monaco.contribution'),
  kotlin: () => import('monaco-editor/esm/vs/basic-languages/kotlin/kotlin.contribution'),
  less: () => import('monaco-editor/esm/vs/basic-languages/less/less.contribution'),
  lua: () => import('monaco-editor/esm/vs/basic-languages/lua/lua.contribution'),
  markdown: () => import('monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution'),
  php: () => import('monaco-editor/esm/vs/basic-languages/php/php.contribution'),
  protobuf: () => import('monaco-editor/esm/vs/basic-languages/protobuf/protobuf.contribution'),
  python: () => import('monaco-editor/esm/vs/basic-languages/python/python.contribution'),
  ruby: () => import('monaco-editor/esm/vs/basic-languages/ruby/ruby.contribution'),
  rust: () => import('monaco-editor/esm/vs/basic-languages/rust/rust.contribution'),
  scss: () => import('monaco-editor/esm/vs/basic-languages/scss/scss.contribution'),
  shell: () => import('monaco-editor/esm/vs/basic-languages/shell/shell.contribution'),
  sql: () => import('monaco-editor/esm/vs/basic-languages/sql/sql.contribution'),
  swift: () => import('monaco-editor/esm/vs/basic-languages/swift/swift.contribution'),
  typescript: () =>
    import('monaco-editor/esm/vs/basic-languages/typescript/typescript.contribution'),
  xml: () => import('monaco-editor/esm/vs/basic-languages/xml/xml.contribution'),
  yaml: () => import('monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution'),
};

export function isEditorWriteShortcut(
  event: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey'>,
): boolean {
  return (event.metaKey || event.ctrlKey) && ['s', 'v', 'x'].includes(event.key.toLowerCase());
}
