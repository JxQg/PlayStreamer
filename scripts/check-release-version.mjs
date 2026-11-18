import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const tag = process.argv[2];
if (typeof tag !== 'string' || tag !== tag.trim() || !/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-rc\.(0|[1-9]\d*))?$/.test(tag)) {
  throw new Error('Release tag must be vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-rc.N.');
}
const expected = tag.slice(1);
const packageJson = JSON.parse(await readFile(path.join(root, 'package.json'), 'utf8'));
const tauriConfig = JSON.parse(await readFile(path.join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
const cargo = await readFile(path.join(root, 'Cargo.toml'), 'utf8');
const workspaceSection = cargo.match(/\[workspace\.package\]([\s\S]*?)(?=\n\[|$)/)?.[1];
const cargoVersion = workspaceSection?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
for (const [source, actual] of Object.entries({ 'package.json': packageJson.version, 'src-tauri/tauri.conf.json': tauriConfig.version, 'Cargo.toml': cargoVersion })) {
  if (actual !== expected) throw new Error(`${source} version ${actual} does not match ${tag}.`);
}
console.log(`All application versions match ${tag}.`);
