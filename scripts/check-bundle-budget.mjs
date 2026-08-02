import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const buildDirectory = path.resolve('build');
const clientBuildDirectory = path.resolve('.svelte-kit/output/client');
const clientManifest = JSON.parse(
  await readFile(path.join(clientBuildDirectory, '.vite/manifest.json'), 'utf8'),
);
const files = await collectFiles(buildDirectory);
const javascript = files.filter((file) => file.path.endsWith('.js'));
const stylesheets = files.filter((file) => file.path.endsWith('.css'));
const totalJavascript = sum(javascript);
const totalCss = sum(stylesheets);
const initialEntries = Object.keys(clientManifest).filter((key) =>
  ['entry/start', 'entry/app', 'nodes/0', 'nodes/1', 'nodes/2'].includes(clientManifest[key].name),
);
if (initialEntries.length !== 5) {
  throw new Error(`Expected five initial client entries; found ${initialEntries.length}`);
}
const initialManifestEntries = collectManifestEntries(clientManifest, initialEntries);
const initialJavascript = await sumManifestFiles(
  clientManifest,
  initialManifestEntries,
  clientBuildDirectory,
  'file',
);
const initialCss = await sumManifestFiles(
  clientManifest,
  initialManifestEntries,
  clientBuildDirectory,
  'css',
);
const largestJavascript = javascript.reduce((largest, file) =>
  !largest || file.bytes > largest.bytes ? file : largest,
);

const limits = {
  // Measured at 309 KiB (316,618 bytes) after lazy-loading the onboarding and
  // restore-error views; keep ~12% headroom per
  // .agents/05-development-standards.md.
  initialJavascript: 350 * 1024,
  // Measured at 72.5 KiB (74,239 bytes) for the same reason.
  initialCss: 82 * 1024,
  totalJavascript: 12 * 1024 * 1024,
  largestJavascript: 6 * 1024 * 1024,
  totalCss: 512 * 1024,
};

assertWithin('initial JavaScript', initialJavascript, limits.initialJavascript);
assertWithin('initial CSS', initialCss, limits.initialCss);
assertWithin('total JavaScript', totalJavascript, limits.totalJavascript);
assertWithin('largest JavaScript chunk', largestJavascript?.bytes || 0, limits.largestJavascript);
assertWithin('total CSS', totalCss, limits.totalCss);

console.log(
  JSON.stringify({
    initialJavascript,
    initialCss,
    totalJavascript,
    largestJavascript: largestJavascript
      ? {
          path: path.relative(buildDirectory, largestJavascript.path),
          bytes: largestJavascript.bytes,
        }
      : null,
    totalCss,
  }),
);

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) return collectFiles(entryPath);
      if (!entry.isFile()) return [];
      return [{ path: entryPath, bytes: (await stat(entryPath)).size }];
    }),
  );
  return nested.flat();
}

function sum(files) {
  return files.reduce((total, file) => total + file.bytes, 0);
}

function collectManifestEntries(manifest, roots) {
  const entries = new Set();
  const visit = (key) => {
    if (entries.has(key)) return;
    entries.add(key);
    for (const dependency of manifest[key]?.imports ?? []) visit(dependency);
  };
  for (const root of roots) visit(root);
  return entries;
}

async function sumManifestFiles(manifest, entries, directory, field) {
  const relativePaths = new Set();
  for (const key of entries) {
    const value = manifest[key]?.[field];
    if (typeof value === 'string') relativePaths.add(value);
    else if (Array.isArray(value)) for (const file of value) relativePaths.add(file);
  }
  const fileStats = await Promise.all(
    [...relativePaths].map(async (file) => ({
      bytes: (await stat(path.join(directory, file))).size,
    })),
  );
  return sum(fileStats);
}

function assertWithin(label, actual, maximum) {
  if (actual > maximum) {
    throw new Error(`${label} is ${actual} bytes; budget is ${maximum} bytes`);
  }
}
