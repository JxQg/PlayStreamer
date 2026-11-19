import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { copyFile, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const workflow = await readFile(path.join(root, '.github/workflows/release.yml'), 'utf8');
const scripts = [...workflow.matchAll(/^          script: \|\r?\n((?:^            .*\r?\n|^\r?\n)*)/gm)]
  .map((match) => match[1].split(/\r?\n/).map((line) => line.slice(12)).join('\n'));
assert.equal(scripts.length, 2, 'release workflow embeds validation and publish scripts');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
const validate = new AsyncFunction('github', 'context', 'core', 'process', scripts[0]);
const publish = new AsyncFunction('github', 'context', 'require', 'process', scripts[1]);

function fixture({ existing = false, annotated = false } = {}) {
  const outputs = {};
  const github = { rest: {
    git: {
      getRef: async () => ({ data: { object: annotated ? { type: 'tag', sha: 'tag-object' } : { type: 'commit', sha: 'commit-sha' } } }),
      getTag: async () => ({ data: { object: { type: 'commit', sha: 'commit-sha' } } }),
    },
    repos: { getReleaseByTag: async () => { if (existing) return { data: {} }; throw Object.assign(new Error('Not found'), { status: 404 }); } },
  } };
  return { github, context: { repo: { owner: 'example', repo: 'playstreamer' } }, core: { setOutput: (key, value) => { outputs[key] = value; } }, outputs };
}

for (const [tag, annotated] of [['v0.1.0', false], ['v0.2.0-rc.1', true]]) {
  test(`release validation resolves ${tag}`, async () => {
    const f = fixture({ annotated });
    await validate(f.github, f.context, f.core, { env: { REQUESTED_TAG: tag } });
    assert.deepEqual(f.outputs, { tag, commit: 'commit-sha', prerelease: tag.includes('-rc.') });
  });
}

test('existing releases are never replaced', async () => {
  const f = fixture({ existing: true });
  await assert.rejects(validate(f.github, f.context, f.core, { env: { REQUESTED_TAG: 'v0.1.0' } }), /already exists/);
});

for (const tag of ['v0.1.0;echo injected', 'v0.1.0\nmalicious', 'v0.1.0\n', 'v01.1.0', 'main', 'v0.1.0-rc.01']) {
  test(`invalid release input is rejected: ${JSON.stringify(tag)}`, async () => {
    const f = fixture();
    f.github.rest.git.getRef = async () => { throw new Error('must not access API for invalid input'); };
    await assert.rejects(validate(f.github, f.context, f.core, { env: { REQUESTED_TAG: tag } }), /Tag must be/);
  });
}

function publishFixture({ uploadFails = false, movedTag = false, badChecksum = false } = {}) {
  const f = fixture();
  const operations = [];
  const installer = Buffer.from('installer bytes');
  const name = 'PlayStreamer_0.1.0_windows_x64_setup.exe';
  const digest = createHash('sha256').update(installer).digest('hex');
  const assets = { [name]: installer, SHA256SUMS: `${badChecksum ? 'wrong' : digest}  ${name}\n`, 'RELEASE_NOTES.md': '# Notes\n' };
  const fs = { readdir: async () => Object.keys(assets), readFile: async (file) => assets[path.posix.basename(file)] };
  Object.assign(f.github.rest.repos, {
    createRelease: async (args) => { operations.push(['create', args]); return { data: { id: 1 } }; },
    uploadReleaseAsset: async (args) => { operations.push(['upload', args.name]); if (uploadFails) throw new Error('upload failed'); },
    updateRelease: async (args) => { operations.push(['publish', args]); },
  });
  const require = (module) => module === 'node:fs/promises' ? fs : { createHash };
  const process = { env: { RELEASE_TAG: 'v0.1.0', RELEASE_COMMIT: movedTag ? 'different-commit' : 'commit-sha', IS_PRERELEASE: 'false' } };
  return { ...f, operations, require, process };
}

test('release becomes public only after every verified asset is uploaded', async () => {
  const f = publishFixture();
  await publish(f.github, f.context, f.require, f.process);
  assert.deepEqual(f.operations.map(([operation]) => operation), ['create', 'upload', 'upload', 'upload', 'publish']);
  assert.equal(f.operations[0][1].draft, true);
  assert.equal(f.operations.at(-1)[1].draft, false);
});

test('upload failure leaves the release draft', async () => {
  const f = publishFixture({ uploadFails: true });
  await assert.rejects(publish(f.github, f.context, f.require, f.process), /upload failed/);
  assert.equal(f.operations.some(([operation]) => operation === 'publish'), false);
});

for (const [option, error] of [['movedTag', /tag moved/], ['badChecksum', /checksum mismatch/]]) {
  test(`${option} prevents release creation`, async () => {
    const f = publishFixture({ [option]: true });
    await assert.rejects(publish(f.github, f.context, f.require, f.process), error);
    assert.equal(f.operations.length, 0);
  });
}

test('release packaging writes verified assets and refuses mixed output', async () => {
  const temporary = await mkdtemp(path.join(tmpdir(), 'playstreamer-release-test-'));
  try {
    await mkdir(path.join(temporary, 'scripts'), { recursive: true });
    await mkdir(path.join(temporary, 'docs/releases'), { recursive: true });
    await writeFile(path.join(temporary, 'docs/releases/0.1.0.md'), 'Security release fixture.\n');
    await mkdir(path.join(temporary, 'target/release/bundle/nsis'), { recursive: true });
    await copyFile(path.join(root, 'scripts/package-release.mjs'), path.join(temporary, 'scripts/package-release.mjs'));
    await writeFile(path.join(temporary, 'package.json'), JSON.stringify({ version: '0.1.0' }));
    const content = Buffer.from('fixture installer');
    await writeFile(path.join(temporary, 'target/release/bundle/nsis/fixture-setup.exe'), content);
    const run = () => spawnSync(process.execPath, ['scripts/package-release.mjs'], { cwd: temporary, encoding: 'utf8' });
    const first = run();
    assert.equal(first.status, 0, first.stderr);
    const digest = createHash('sha256').update(content).digest('hex');
    assert.equal(await readFile(path.join(temporary, 'dist-release/SHA256SUMS'), 'utf8'), `${digest}  PlayStreamer_0.1.0_windows_x64_setup.exe\n`);
    const notes = await readFile(path.join(temporary, 'dist-release/RELEASE_NOTES.md'), 'utf8');
    assert.match(notes, /^# PlayStreamer 0\.1\.0\n\n/);
    assert.match(notes, /Security release fixture\./);
    const second = run();
    assert.notEqual(second.status, 0);
    assert.match(second.stderr, /dist-release must be empty/);
  } finally {
    await rm(temporary, { recursive: true, force: true });
  }
});
