import type { SourceRef } from './entities';

export type SourceTreeFile = {
  kind: 'file';
  id: string;
  name: string;
  path: string;
  refs: SourceRef[];
};

export type SourceTreeFolder = {
  kind: 'folder';
  id: string;
  name: string;
  path: string;
  children: SourceTreeNode[];
};

export type SourceTreeNode = SourceTreeFolder | SourceTreeFile;

type MutableFolder = {
  name: string;
  path: string;
  folders: Map<string, MutableFolder>;
  files: Map<string, SourceRef[]>;
};

export function buildSourceTree(sourceRefs: SourceRef[]): SourceTreeNode[] {
  const root: MutableFolder = { name: '', path: '', folders: new Map(), files: new Map() };

  for (const sourceRef of sourceRefs) {
    const parts = sourceRef.path.split('/').filter(Boolean);
    const fileName = parts.pop();
    if (!fileName) continue;

    let folder = root;
    for (const part of parts) {
      const path = folder.path ? `${folder.path}/${part}` : part;
      let child = folder.folders.get(part);
      if (!child) {
        child = { name: part, path, folders: new Map(), files: new Map() };
        folder.folders.set(part, child);
      }
      folder = child;
    }

    const refs = folder.files.get(fileName) ?? [];
    refs.push(sourceRef);
    folder.files.set(fileName, refs);
  }

  return materialize(root);
}

export function sourceTreeFileCount(nodes: SourceTreeNode[]): number {
  return nodes.reduce(
    (count, node) =>
      count + (node.kind === 'file' ? 1 : sourceTreeFileCount(node.children)),
    0,
  );
}

function materialize(folder: MutableFolder): SourceTreeNode[] {
  const folders = [...folder.folders.values()]
    .sort((left, right) => compareNames(left.name, right.name))
    .map<SourceTreeFolder>((child) => ({
      kind: 'folder',
      id: `folder:${child.path}`,
      name: child.name,
      path: child.path,
      children: materialize(child),
    }));

  const files = [...folder.files.entries()]
    .sort(([left], [right]) => compareNames(left, right))
    .map<SourceTreeFile>(([name, refs]) => {
      const path = folder.path ? `${folder.path}/${name}` : name;
      return {
        kind: 'file',
        id: `file:${path}`,
        name,
        path,
        refs: refs.toSorted(
          (left, right) => left.startLine - right.startLine || left.endLine - right.endLine,
        ),
      };
    });

  return [...folders, ...files];
}

function compareNames(left: string, right: string): number {
  return left.localeCompare(right, undefined, { numeric: true, sensitivity: 'base' });
}
