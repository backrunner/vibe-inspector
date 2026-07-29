import { chmod, copyFile, mkdir } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = path.join(root, 'src-tauri', 'Cargo.toml');
const release = process.argv.includes('--release');
const target = process.env.TAURI_ENV_TARGET_TRIPLE || (await rustHostTarget());
const profile = release ? 'release' : 'debug';
const sidecars = ['vibe-inspector-tray', 'vibe-inspector-mcp'];
const cargoArguments = ['build', '--manifest-path', manifest];
for (const sidecar of sidecars) cargoArguments.push('--bin', sidecar);

if (release) cargoArguments.push('--release');
if (process.env.TAURI_ENV_TARGET_TRIPLE) {
  cargoArguments.push('--target', process.env.TAURI_ENV_TARGET_TRIPLE);
}

await run('cargo', cargoArguments);

const destinationDirectory = path.join(root, 'src-tauri', 'binaries');
await mkdir(destinationDirectory, { recursive: true });
for (const sidecar of sidecars) {
  const targetParts = [root, 'src-tauri', 'target'];
  if (process.env.TAURI_ENV_TARGET_TRIPLE) targetParts.push(target);
  targetParts.push(profile, sidecar);

  const source = path.join(...targetParts);
  const destination = path.join(destinationDirectory, `${sidecar}-${target}`);
  await copyFile(source, destination);
  await chmod(destination, 0o755);
}

async function rustHostTarget() {
  const output = await capture('rustc', ['-vV']);
  const host = output
    .split('\n')
    .find((line) => line.startsWith('host: '))
    ?.slice('host: '.length)
    .trim();
  if (!host) throw new Error('rustc did not report a host target');
  return host;
}

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: root, stdio: 'inherit' });
    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (code === 0) resolve();
      else reject(new Error(`${command} failed with ${signal || `exit code ${code}`}`));
    });
  });
}

function capture(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { cwd: root, stdio: ['ignore', 'pipe', 'inherit'] });
    let output = '';
    child.stdout.setEncoding('utf8');
    child.stdout.on('data', (chunk) => {
      output += chunk;
    });
    child.on('error', reject);
    child.on('exit', (code, signal) => {
      if (code === 0) resolve(output);
      else reject(new Error(`${command} failed with ${signal || `exit code ${code}`}`));
    });
  });
}
