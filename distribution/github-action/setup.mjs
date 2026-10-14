import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFile, mkdir, mkdtemp, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';

const execute = promisify(execFile);
const targets = new Map([
  ['linux-x64', 'x86_64-unknown-linux-gnu'],
  ['linux-arm64', 'aarch64-unknown-linux-gnu'],
  ['darwin-x64', 'x86_64-apple-darwin'],
  ['darwin-arm64', 'aarch64-apple-darwin'],
  ['win32-x64', 'x86_64-pc-windows-msvc'],
]);

export function artifactFor(platform, arch) {
  const target = targets.get(`${platform}-${arch}`);
  if (!target) throw new Error(`Unsupported runner platform: ${platform}/${arch}`);
  const stem = `merman-cli-${target}`;
  return {
    target,
    archive: `${stem}${platform === 'win32' ? '.zip' : '.tar.xz'}`,
    binary: platform === 'win32' ? 'merman-cli.exe' : `${stem}/merman-cli`,
  };
}

export function releaseFor(version, platform, arch) {
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(version);
  if (!match || match[0] !== version || match[4]?.split('.').some((part) => /^0\d+$/.test(part))) {
    throw new Error('version must be an exact release version without the v prefix, such as 0.9.0 or 0.8.0-alpha.7');
  }
  const artifact = artifactFor(platform, arch);
  return {
    ...artifact,
    version,
    url: `https://github.com/Latias94/merman/releases/download/v${version}/${artifact.archive}`,
  };
}

async function responseFor(url, download) {
  const response = await download(url);
  if (!response.ok) throw new Error(`Download failed (HTTP ${response.status}): ${url}`);
  return response;
}

export async function setup({ version, runnerTemp, githubPath, githubOutput, platform = process.platform, arch = process.arch }, download = fetch) {
  const release = releaseFor(version, platform, arch);
  if (!runnerTemp || !githubPath || !githubOutput) {
    throw new Error('RUNNER_TEMP, GITHUB_PATH and GITHUB_OUTPUT must be set by the GitHub Actions runner');
  }
  const checksumText = await (await responseFor(`${release.url}.sha256`, download)).text();
  const checksum = /^(?:sha256:)?([0-9A-Fa-f]{64})(?:[ \t]+\*?([0-9A-Za-z._-]+))?\r?\n?$/.exec(checksumText);
  if (!checksum || checksum[0] !== checksumText || (checksum[2] && checksum[2] !== release.archive)) {
    throw new Error(`Invalid SHA-256 checksum for ${release.archive}`);
  }
  const bytes = Buffer.from(await (await responseFor(release.url, download)).arrayBuffer());
  if (createHash('sha256').update(bytes).digest('hex') !== checksum[1].toLowerCase()) {
    throw new Error(`SHA-256 mismatch for ${release.archive}`);
  }

  const installRoot = await mkdtemp(join(resolve(runnerTemp), 'merman-cli-'));
  const archivePath = join(installRoot, release.archive);
  const payload = join(installRoot, 'payload');
  await writeFile(archivePath, bytes);
  await mkdir(payload);
  const tar = process.platform === 'win32' ? join(process.env.SystemRoot, 'System32', 'tar.exe') : 'tar';
  await execute(tar, ['-xf', archivePath, '-C', payload]);
  const binary = join(payload, release.binary);
  if (!(await stat(binary)).isFile()) throw new Error(`Release archive is missing ${release.binary}`);
  await appendFile(githubPath, `${dirname(binary)}\n`);
  await appendFile(githubOutput, `version=${version}\npath=${binary}\n`);
  return { version, path: binary };
}
