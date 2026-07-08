#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";

const vsceTargets = new Set([
  "win32-x64",
  "win32-arm64",
  "linux-x64",
  "linux-arm64",
  "linux-armhf",
  "darwin-x64",
  "darwin-arm64",
  "alpine-x64",
  "alpine-arm64",
  "web",
]);

const args = parseArgs(
  normalizeNpmForwardedArgs(process.argv.slice(2), [
    "vsix",
    "platform",
    "target",
    "publisher",
    "version",
  ]),
);
const vsixPath = args.vsix ?? findDefaultVsix();
const platformKey = args.platform ?? `${process.platform}-${process.arch}`;
const executableSuffix = platformKey.startsWith("win32-") ? ".exe" : "";
const expectedTarget = args.target ?? args.platform ?? null;
const expectedPublisher = args.publisher ?? "latias94";
const sourceManifest = JSON.parse(fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"));
const expectedSourceVersion =
  args.version ??
  process.env.MERMAN_RELEASE_VERSION ??
  readWorkspacePackageVersion() ??
  sourceManifest.version ??
  null;
const expectedVersion = expectedSourceVersion === null ? null : parseSourceVersion(expectedSourceVersion);

if (!vsixPath) {
  fail("Missing --vsix <path> and no .vsix file was found in the current directory.");
}

const zip = readZip(vsixPath);
const entries = new Set(zip.entries.map((entry) => entry.name));
const requiredEntries = [
  "extension/package.json",
  "extension.vsixmanifest",
  "extension/readme.md",
  "extension/changelog.md",
  "extension/LICENSE.txt",
  "extension/dist/extension.js",
  "extension/media/preview.css",
  "extension/media/preview.js",
  "extension/snippets/mermaid.json",
  `extension/bin/${platformKey}/merman-lsp${executableSuffix}`,
  `extension/bin/${platformKey}/merman-cli${executableSuffix}`,
];

const missing = requiredEntries.filter((entry) => !entries.has(entry));
if (missing.length > 0) {
  fail(`VSIX is missing required entries:\n${missing.map((entry) => `- ${entry}`).join("\n")}`);
}

const forbiddenPrefixes = [
  "extension/src/",
  "extension/scripts/",
  "extension/node_modules/",
  "extension/dist/test/",
];
const forbiddenSuffixes = [".vsix"];
const forbidden = [...entries].filter(
  (entry) =>
    forbiddenPrefixes.some((prefix) => entry.startsWith(prefix)) ||
    forbiddenSuffixes.some((suffix) => entry.endsWith(suffix)),
);
if (forbidden.length > 0) {
  fail(`VSIX includes files that should not be published:\n${forbidden.map((entry) => `- ${entry}`).join("\n")}`);
}

const manifest = JSON.parse(zip.readText("extension/package.json"));
const vsixManifestXml = zip.readText("extension.vsixmanifest");
assertEqual(manifest.name, "merman-vscode", "package name");
assertEqual(manifest.publisher, expectedPublisher, "publisher");
assertEqual(manifest.preview, true, "preview flag");
assertEqual(manifest.qna, false, "qna flag");
if (expectedVersion !== null) {
  assertEqual(manifest.version, expectedVersion.vsixVersion, "version");
  assertEqual(readVsixIdentityVersion(vsixManifestXml), expectedVersion.vsixVersion, "VSIX identity version");
  assertEqual(hasVsixPreReleaseProperty(vsixManifestXml), expectedVersion.preRelease, "VSIX pre-release marker");
}
if (parseSourceVersion(manifest.version).preRelease) {
  fail(`VSIX package.json version must be major.minor.patch, got ${JSON.stringify(manifest.version)}.`);
}
if (manifest.private !== undefined) {
  fail("VSIX package.json must not include private=true.");
}
if (!manifest.repository?.url?.includes("github.com/Latias94/merman")) {
  fail("VSIX package.json is missing the expected repository URL.");
}
if (!manifest.bugs?.url?.includes("github.com/Latias94/merman/issues")) {
  fail("VSIX package.json is missing the expected bugs URL.");
}
if (expectedTarget !== null) {
  const targetPlatform = readVsixTargetPlatform(vsixManifestXml);
  assertEqual(targetPlatform, expectedTarget, "VSIX target platform");
}

console.log(
  `verified ${path.basename(vsixPath)}: ${entries.size} entries, platform=${platformKey}, version=${manifest.version}, preRelease=${hasVsixPreReleaseProperty(vsixManifestXml)}`,
);

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--vsix") {
      parsed.vsix = argv[++index];
    } else if (arg.startsWith("--vsix=")) {
      parsed.vsix = arg.slice("--vsix=".length);
    } else if (arg === "--platform") {
      parsed.platform = argv[++index];
    } else if (arg.startsWith("--platform=")) {
      parsed.platform = arg.slice("--platform=".length);
    } else if (arg === "--target") {
      parsed.target = argv[++index];
    } else if (arg.startsWith("--target=")) {
      parsed.target = arg.slice("--target=".length);
    } else if (arg === "--publisher") {
      parsed.publisher = argv[++index];
    } else if (arg.startsWith("--publisher=")) {
      parsed.publisher = arg.slice("--publisher=".length);
    } else if (arg === "--version") {
      parsed.version = argv[++index];
    } else if (arg.startsWith("--version=")) {
      parsed.version = arg.slice("--version=".length);
    } else if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    } else {
      fail(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}

function normalizeNpmForwardedArgs(args, options) {
  let normalized = [...args];
  for (const option of options) {
    const value = process.env[`npm_config_${option}`];
    const flag = `--${option}`;
    if (!value || value === "true" || hasOption(normalized, flag)) {
      continue;
    }
    normalized = normalized.filter((arg) => arg !== value);
    normalized.push(flag, value);
  }
  normalized = normalizeBareForwardedArgs(normalized);
  return normalized;
}

function normalizeBareForwardedArgs(args) {
  const normalized = [];
  for (const arg of args) {
    if (arg.startsWith("-")) {
      normalized.push(arg);
    } else if (arg.endsWith(".vsix") && !hasOption([...normalized, ...args], "--vsix")) {
      normalized.push("--vsix", arg);
    } else if (vsceTargets.has(arg) && !hasOption([...normalized, ...args], "--platform")) {
      normalized.push("--platform", arg);
    } else if (vsceTargets.has(arg) && !hasOption([...normalized, ...args], "--target")) {
      normalized.push("--target", arg);
    } else if (isSemVerLike(arg) && !hasOption([...normalized, ...args], "--version")) {
      normalized.push("--version", arg);
    } else {
      normalized.push(arg);
    }
  }
  return normalized;
}

function hasOption(args, flag) {
  return args.some((arg) => arg === flag || arg.startsWith(`${flag}=`));
}

function isSemVerLike(value) {
  return /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(value);
}

function findDefaultVsix() {
  const files = fs.readdirSync(process.cwd()).filter((file) => file.endsWith(".vsix"));
  return files.length === 1 ? files[0] : null;
}

function readZip(filePath) {
  const buffer = fs.readFileSync(filePath);
  const eocdOffset = findEndOfCentralDirectory(buffer);
  const entryCount = buffer.readUInt16LE(eocdOffset + 10);
  const centralDirectorySize = buffer.readUInt32LE(eocdOffset + 12);
  const centralDirectoryOffset = buffer.readUInt32LE(eocdOffset + 16);
  const entries = [];
  let cursor = centralDirectoryOffset;
  const end = centralDirectoryOffset + centralDirectorySize;

  for (let index = 0; index < entryCount && cursor < end; index += 1) {
    if (buffer.readUInt32LE(cursor) !== 0x02014b50) {
      fail(`Invalid ZIP central directory at offset ${cursor}.`);
    }
    const method = buffer.readUInt16LE(cursor + 10);
    const compressedSize = buffer.readUInt32LE(cursor + 20);
    const uncompressedSize = buffer.readUInt32LE(cursor + 24);
    const nameLength = buffer.readUInt16LE(cursor + 28);
    const extraLength = buffer.readUInt16LE(cursor + 30);
    const commentLength = buffer.readUInt16LE(cursor + 32);
    const localHeaderOffset = buffer.readUInt32LE(cursor + 42);
    const name = buffer
      .subarray(cursor + 46, cursor + 46 + nameLength)
      .toString("utf8")
      .replaceAll("\\", "/");
    entries.push({
      name,
      method,
      compressedSize,
      uncompressedSize,
      localHeaderOffset,
    });
    cursor += 46 + nameLength + extraLength + commentLength;
  }

  return {
    entries,
    readText(name) {
      const entry = entries.find((candidate) => candidate.name === name);
      if (!entry) {
        fail(`VSIX entry not found: ${name}`);
      }
      return readEntryData(buffer, entry).toString("utf8");
    },
  };
}

function findEndOfCentralDirectory(buffer) {
  const minimumOffset = Math.max(0, buffer.length - 22 - 0xffff);
  for (let offset = buffer.length - 22; offset >= minimumOffset; offset -= 1) {
    if (buffer.readUInt32LE(offset) === 0x06054b50) {
      return offset;
    }
  }
  fail("Invalid VSIX: could not find ZIP end of central directory.");
}

function readEntryData(buffer, entry) {
  const cursor = entry.localHeaderOffset;
  if (buffer.readUInt32LE(cursor) !== 0x04034b50) {
    fail(`Invalid ZIP local header for ${entry.name}.`);
  }
  const nameLength = buffer.readUInt16LE(cursor + 26);
  const extraLength = buffer.readUInt16LE(cursor + 28);
  const dataStart = cursor + 30 + nameLength + extraLength;
  const compressed = buffer.subarray(dataStart, dataStart + entry.compressedSize);
  if (entry.method === 0) {
    return compressed;
  }
  if (entry.method === 8) {
    const inflated = zlib.inflateRawSync(compressed);
    if (inflated.length !== entry.uncompressedSize) {
      fail(`Invalid uncompressed size for ${entry.name}.`);
    }
    return inflated;
  }
  fail(`Unsupported ZIP compression method ${entry.method} for ${entry.name}.`);
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    fail(`Unexpected ${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}.`);
  }
}

function parseSourceVersion(version) {
  const match = version.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) {
    fail(`Expected source version is not valid SemVer: ${JSON.stringify(version)}.`);
  }
  return {
    sourceVersion: version,
    vsixVersion: `${match[1]}.${match[2]}.${match[3]}`,
    preRelease: match[4] !== undefined,
  };
}

function readWorkspacePackageVersion() {
  const cargoTomlPath = path.resolve(process.cwd(), "..", "..", "Cargo.toml");
  if (!fs.existsSync(cargoTomlPath)) {
    return null;
  }
  const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
  const match = cargoToml.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  return match?.[1] ?? null;
}

function readVsixIdentityVersion(manifestXml) {
  const match = manifestXml.match(/<Identity\b[^>]*\bVersion=(["'])([^"']+)\1/);
  if (!match) {
    fail("VSIX manifest is missing Identity Version.");
  }
  return match[2];
}

function readVsixTargetPlatform(manifestXml) {
  const match = manifestXml.match(/\bTargetPlatform=(["'])([^"']+)\1/);
  if (!match) {
    fail("VSIX manifest is missing TargetPlatform.");
  }
  return match[2];
}

function hasVsixPreReleaseProperty(manifestXml) {
  const properties = manifestXml.match(/<Property\b[^>]*>/g) ?? [];
  for (const property of properties) {
    if (!/\bId=(["'])Microsoft\.VisualStudio\.Code\.PreRelease\1/.test(property)) {
      continue;
    }
    if (!/\bValue=(["'])true\1/.test(property)) {
      fail("VSIX manifest has a pre-release property with a non-true value.");
    }
    return true;
  }
  return false;
}

function printUsage() {
  console.log(
    "usage: node scripts/verify-vsix.mjs --vsix <path> [--platform <platform-arch>] [--target <platform-arch>] [--version <source-version>]",
  );
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
