import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(await readFile(path.join(root, 'scripts/sidecar-manifest.json'), 'utf8'));
const target = process.argv[2] ?? 'x86_64-pc-windows-msvc';
const entry = manifest.targets[target];
if (!entry || process.platform !== 'win32') {
  throw new Error(`Only Windows x64 sidecar preparation is supported; received ${target} on ${process.platform}.`);
}
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const cache = path.join(root, '.cache', 'mediamtx');
const archivePath = path.join(cache, entry.archive);
await mkdir(cache, { recursive: true });
let archive;
try {
  archive = await readFile(archivePath);
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}
if (archive && hash(archive) !== entry.sha256) {
  throw new Error(`Cached MediaMTX archive failed SHA256 verification. Remove ${archivePath} and retry.`);
}
if (!archive) {
  const url = `${manifest.source}/releases/download/v${manifest.version}/${entry.archive}`;
  console.log(`Downloading ${manifest.name} ${manifest.version} (${target})`);
  const response = await fetch(url, { signal: AbortSignal.timeout(180_000) });
  if (!response.ok) throw new Error(`MediaMTX download failed: HTTP ${response.status}`);
  archive = Buffer.from(await response.arrayBuffer());
  if (hash(archive) !== entry.sha256) throw new Error('MediaMTX download failed pinned SHA256 verification.');
  const temporaryArchive = `${archivePath}.${process.pid}.tmp`;
  await writeFile(temporaryArchive, archive);
  await rename(temporaryArchive, archivePath);
}

const temporaryDirectory = await mkdtemp(path.join(tmpdir(), 'playstreamer-mediamtx-'));
try {
  // Extract only known members from the archive verified against the pinned upstream digest.
  execFileSync('tar.exe', ['-xf', archivePath, '-C', temporaryDirectory, entry.executable, 'LICENSE'], { stdio: 'inherit' });
  const binaryDirectory = path.join(root, 'src-tauri', 'binaries');
  const resourceDirectory = path.join(root, 'src-tauri', 'resources');
  await mkdir(binaryDirectory, { recursive: true });
  await mkdir(resourceDirectory, { recursive: true });
  const destination = path.join(binaryDirectory, `mediamtx-${target}.exe`);
  await copyFile(path.join(temporaryDirectory, entry.executable), destination);
  await copyFile(path.join(temporaryDirectory, 'LICENSE'), path.join(resourceDirectory, 'mediamtx-LICENSE'));
  console.log(`Verified SHA256 ${entry.sha256}`);
  console.log(`Prepared ${path.relative(root, destination)}`);
} finally {
  await rm(temporaryDirectory, { recursive: true, force: true });
}
