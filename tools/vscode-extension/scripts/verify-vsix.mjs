#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";

const args = parseArgs(process.argv.slice(2));
const vsixPath = args.vsix ?? findDefaultVsix();
const platformKey = args.platform ?? `${process.platform}-${process.arch}`;
const executableSuffix = platformKey.startsWith("win32-") ? ".exe" : "";
const expectedTarget = args.target ?? args.platform ?? null;
const expectedPublisher = args.publisher ?? "latias94";
const expectedVersion = args.version ?? null;

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
assertEqual(manifest.name, "merman-vscode", "package name");
assertEqual(manifest.publisher, expectedPublisher, "publisher");
assertEqual(manifest.preview, true, "preview flag");
assertEqual(manifest.qna, false, "qna flag");
if (expectedVersion !== null) {
  assertEqual(manifest.version, expectedVersion, "version");
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
  const targetPlatform = readVsixTargetPlatform(zip.readText("extension.vsixmanifest"));
  assertEqual(targetPlatform, expectedTarget, "VSIX target platform");
}

console.log(
  `verified ${path.basename(vsixPath)}: ${entries.size} entries, platform=${platformKey}, version=${manifest.version}`,
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

function readVsixTargetPlatform(manifestXml) {
  const match = manifestXml.match(/\bTargetPlatform=(["'])([^"']+)\1/);
  if (!match) {
    fail("VSIX manifest is missing TargetPlatform.");
  }
  return match[2];
}

function printUsage() {
  console.log("usage: node scripts/verify-vsix.mjs --vsix <path> [--platform <platform-arch>] [--target <platform-arch>]");
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
