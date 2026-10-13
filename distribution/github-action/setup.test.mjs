import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFile, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { promisify } from 'node:util';
import { artifactFor, releaseFor, setup } from './setup.mjs';

const execute = promisify(execFile);
const version = '0.9.0';

test('maps only the five published CLI targets', () => {
  for (const [platform, arch, target, extension] of [
    ['linux', 'x64', 'x86_64-unknown-linux-gnu', '.tar.xz'],
    ['linux', 'arm64', 'aarch64-unknown-linux-gnu', '.tar.xz'],
    ['darwin', 'x64', 'x86_64-apple-darwin', '.tar.xz'],
    ['darwin', 'arm64', 'aarch64-apple-darwin', '.tar.xz'],
    ['win32', 'x64', 'x86_64-pc-windows-msvc', '.zip'],
  ]) {
    assert.equal(artifactFor(platform, arch).archive, `merman-cli-${target}${extension}`);
  }
  for (const [platform, arch] of [['win32', 'arm64'], ['linux', 'ia32'], ['freebsd', 'x64']]) {
    assert.throws(() => artifactFor(platform, arch), /Unsupported runner platform/);
  }
});

test('uses an explicit exact version and the fixed official release source', () => {
  for (const selected of ['0.9.0', '0.8.0-alpha.7', '1.2.3-rc.1']) {
    const release = releaseFor(selected, 'linux', 'x64');
    assert.equal(release.version, selected);
    assert.equal(release.url, `https://github.com/Latias94/merman/releases/download/v${selected}/merman-cli-x86_64-unknown-linux-gnu.tar.xz`);
  }
  for (const selected of ['', 'latest', '0.9', 'v0.9.0', '^0.9.0', '01.9.0', '0.9.0-alpha.01', '0.9.0\n', '../0.9.0', '0.9.0;echo injected', 'https://example.com/0.9.0']) {
    assert.throws(() => releaseFor(selected, 'linux', 'x64'), /exact release version/);
  }
});

async function temporary(t) {
  const root = await mkdtemp(join(tmpdir(), 'merman-setup-action-test-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  const runnerTemp = join(root, 'runner');
  await mkdir(runnerTemp);
  const githubPath = join(root, 'github-path');
  const githubOutput = join(root, 'github-output');
  await writeFile(githubPath, 'existing-path\n');
  await writeFile(githubOutput, 'existing-output=value\n');
  return { root, options: { version, runnerTemp, githubPath, githubOutput } };
}

async function releaseServer(t, files) {
  const requests = [];
  const server = createServer((request, response) => {
    requests.push(request.url);
    const bytes = files.get(request.url);
    response.writeHead(bytes === undefined ? 404 : 200);
    response.end(bytes ?? 'not found');
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())));
  const origin = `http://127.0.0.1:${server.address().port}`;
  const download = (url) => {
    const canonical = new URL(url);
    assert.equal(canonical.origin, 'https://github.com');
    assert.match(canonical.pathname, /^\/Latias94\/merman\/releases\/download\/v0\.9\.0\//);
    return fetch(`${origin}${canonical.pathname}`);
  };
  return { download, requests };
}

function assetFiles(archive, bytes, checksum = createHash('sha256').update(bytes).digest('hex')) {
  const prefix = `/Latias94/merman/releases/download/v${version}/`;
  return new Map([
    [`${prefix}${archive}`, bytes],
    [`${prefix}${archive}.sha256`, `${checksum} *${archive}\n`],
  ]);
}

test('rejects corrupted downloaded bytes before extraction or PATH publication', async (t) => {
  const { options } = await temporary(t);
  const artifact = artifactFor(process.platform, process.arch);
  const expectedBytes = Buffer.from('the original archive bytes');
  const corruptedBytes = Buffer.from('the corrupted archive bytes');
  const checksum = createHash('sha256').update(expectedBytes).digest('hex');
  const { download } = await releaseServer(t, assetFiles(artifact.archive, corruptedBytes, checksum));
  await assert.rejects(setup(options, download), /SHA-256 mismatch/);
  assert.deepEqual(await readdir(options.runnerTemp), []);
  assert.equal(await readFile(options.githubPath, 'utf8'), 'existing-path\n');
  assert.equal(await readFile(options.githubOutput, 'utf8'), 'existing-output=value\n');
});

test('rejects malformed or mislabelled checksums', async (t) => {
  const { options } = await temporary(t);
  const artifact = artifactFor(process.platform, process.arch);
  const bytes = Buffer.from('archive bytes');
  const digest = createHash('sha256').update(bytes).digest('hex');
  const files = assetFiles(artifact.archive, bytes);
  const checksumPath = `/Latias94/merman/releases/download/v${version}/${artifact.archive}.sha256`;
  const { download } = await releaseServer(t, files);
  for (const checksum of ['not-a-checksum', `${digest} *another-archive.zip\n`, `${digest}\n${digest}\n`]) {
    files.set(checksumPath, checksum);
    await assert.rejects(setup(options, download), /Invalid SHA-256 checksum/);
  }
  assert.deepEqual(await readdir(options.runnerTemp), []);
});

test('reports an unavailable pinned release without changing PATH', async (t) => {
  const { options } = await temporary(t);
  const { download } = await releaseServer(t, new Map());
  await assert.rejects(setup(options, download), /HTTP 404/);
  assert.equal(await readFile(options.githubPath, 'utf8'), 'existing-path\n');
});

test('downloads, verifies and extracts a real native archive and publishes its executable', async (t) => {
  const { root, options } = await temporary(t);
  const artifact = artifactFor(process.platform, process.arch);
  const payload = join(root, 'source');
  const binary = join(payload, artifact.binary);
  await mkdir(dirname(binary), { recursive: true });
  if (process.platform === 'win32') {
    await copyFile(join(process.env.SystemRoot, 'System32', 'cmd.exe'), binary);
  } else {
    await writeFile(binary, '#!/bin/sh\nprintf "fixture-0.9.0\\n"\n', { mode: 0o755 });
  }
  const archivePath = join(root, artifact.archive);
  await execute('tar', [process.platform === 'win32' ? '-acf' : '-cJf', archivePath, '-C', payload, '.']);
  const archiveBytes = await readFile(archivePath);
  const { download, requests } = await releaseServer(t, assetFiles(artifact.archive, archiveBytes));
  const installed = await setup(options, download);
  assert.equal(installed.version, version);
  assert.equal(await readFile(installed.path, 'base64'), await readFile(binary, 'base64'));
  assert.equal(await readFile(options.githubPath, 'utf8'), `existing-path\n${dirname(installed.path)}\n`);
  assert.equal(await readFile(options.githubOutput, 'utf8'), `existing-output=value\nversion=${version}\npath=${installed.path}\n`);
  assert.equal(requests.length, 2);
  const args = process.platform === 'win32' ? ['/d', '/c', 'echo fixture-0.9.0'] : [];
  const executed = await execute(installed.path, args);
  assert.equal(executed.stdout.trim(), 'fixture-0.9.0');
});
