import type { editor } from 'monaco-editor/editor';

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
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
};

export function readonlyEditorOptions({
  value,
  path,
  theme,
  codeStartLine,
  fontFamily,
  fontSize,
  lineHeight,
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
    lineDecorationsWidth: 8,
    glyphMargin: false,
    folding: true,
    renderLineHighlight: 'all',
    wordWrap: 'off',
    scrollBeyondLastLine: false,
    automaticLayout: true,
    padding: { top: 12, bottom: 18 },
    fontSize,
    lineHeight,
    fontFamily,
    fontLigatures: false,
    fontWeight: '400',
    letterSpacing: 0,
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
  };
}

export function lineNumberMinChars(value: string, codeStartLine: number): number {
  const lineCount = value.split('\n').length;
  const finalLine = Math.max(1, codeStartLine) + lineCount - 1;
  return Math.max(4, String(finalLine).length);
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
  cpp: () => import('monaco-editor/languages/definitions/cpp/register'),
  csharp: () => import('monaco-editor/languages/definitions/csharp/register'),
  css: () => import('monaco-editor/languages/definitions/css/register'),
  dart: () => import('monaco-editor/languages/definitions/dart/register'),
  dockerfile: () => import('monaco-editor/languages/definitions/dockerfile/register'),
  go: () => import('monaco-editor/languages/definitions/go/register'),
  graphql: () => import('monaco-editor/languages/definitions/graphql/register'),
  html: () => import('monaco-editor/languages/definitions/html/register'),
  ini: () => import('monaco-editor/languages/definitions/ini/register'),
  java: () => import('monaco-editor/languages/definitions/java/register'),
  javascript: () => import('monaco-editor/languages/definitions/javascript/register'),
  json: () => import('monaco-editor/languages/features/json/register'),
  kotlin: () => import('monaco-editor/languages/definitions/kotlin/register'),
  less: () => import('monaco-editor/languages/definitions/less/register'),
  lua: () => import('monaco-editor/languages/definitions/lua/register'),
  markdown: () => import('monaco-editor/languages/definitions/markdown/register'),
  php: () => import('monaco-editor/languages/definitions/php/register'),
  protobuf: () => import('monaco-editor/languages/definitions/protobuf/register'),
  python: () => import('monaco-editor/languages/definitions/python/register'),
  ruby: () => import('monaco-editor/languages/definitions/ruby/register'),
  rust: () => import('monaco-editor/languages/definitions/rust/register'),
  scss: () => import('monaco-editor/languages/definitions/scss/register'),
  shell: () => import('monaco-editor/languages/definitions/shell/register'),
  sql: () => import('monaco-editor/languages/definitions/sql/register'),
  swift: () => import('monaco-editor/languages/definitions/swift/register'),
  typescript: () => import('monaco-editor/languages/definitions/typescript/register'),
  xml: () => import('monaco-editor/languages/definitions/xml/register'),
  yaml: () => import('monaco-editor/languages/definitions/yaml/register'),
};

export function isEditorWriteShortcut(
  event: Pick<KeyboardEvent, 'key' | 'metaKey' | 'ctrlKey'>,
): boolean {
  return (event.metaKey || event.ctrlKey) && ['s', 'v', 'x'].includes(event.key.toLowerCase());
}
