#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

if (isMainModule()) {
  try {
    await runSmoke();
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

export async function runSmoke({
  argv = process.argv.slice(2),
  cwd = process.cwd(),
  tempDir = os.tmpdir(),
  testRunner = runTests,
} = {}) {
  const args = parseArgs(argv);
  const vsixPath = args.vsix ?? findDefaultVsix(cwd);

  if (!vsixPath) {
    fail("Missing --vsix <path> and no .vsix file was found in the current directory.");
  }
  if (!fs.existsSync(vsixPath)) {
    fail(`VSIX not found: ${vsixPath}`);
  }

  const tempRoot = fs.mkdtempSync(path.join(tempDir, "merman-vsix-smoke-"));
  try {
    const extensionRoot = path.join(tempRoot, "extension");
    extractVsixExtension(vsixPath, tempRoot);
    if (!fs.existsSync(path.join(extensionRoot, "package.json"))) {
      fail("VSIX did not contain extension/package.json.");
    }

    await testRunner({
      extensionDevelopmentPath: extensionRoot,
      extensionTestsPath: path.join(packageRoot, "dist", "extension-host-smoke.js"),
      launchArgs: [
        path.join(packageRoot, "test-fixtures", "extension-host"),
      ],
    });
    console.log(`packaged VSIX smoke passed: ${path.basename(vsixPath)}`);
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function extractVsixExtension(filePath, destinationRoot) {
  const zip = readZip(filePath);
  for (const entry of zip.entries) {
    if (!entry.name.startsWith("extension/") || entry.name.endsWith("/")) {
      continue;
    }
    const relativePath = entry.name.slice("extension/".length);
    if (!relativePath) {
      continue;
    }
    const destination = path.resolve(destinationRoot, "extension", relativePath);
    const extensionRoot = path.resolve(destinationRoot, "extension");
    if (destination !== extensionRoot && !destination.startsWith(`${extensionRoot}${path.sep}`)) {
      fail(`Unsafe VSIX entry path: ${entry.name}`);
    }
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    const data = readEntryData(zip.buffer, entry);
    const mode = entry.mode || (relativePath.startsWith(`bin/`) ? 0o755 : 0o644);
    fs.writeFileSync(destination, data, { mode });
    if (mode) {
      fs.chmodSync(destination, mode);
    }
  }
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
    const externalAttributes = buffer.readUInt32LE(cursor + 38);
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
      mode: (externalAttributes >>> 16) & 0o777,
    });
    cursor += 46 + nameLength + extraLength + commentLength;
  }

  return { buffer, entries };
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

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--vsix") {
      parsed.vsix = argv[++index];
    } else if (arg.startsWith("--vsix=")) {
      parsed.vsix = arg.slice("--vsix=".length);
    } else if (!arg.startsWith("-") && arg.endsWith(".vsix") && !parsed.vsix) {
      parsed.vsix = arg;
    } else if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    } else {
      fail(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}

function findDefaultVsix(cwd) {
  const files = fs.readdirSync(cwd).filter((file) => file.endsWith(".vsix"));
  return files.length === 1 ? path.join(cwd, files[0]) : null;
}

function printUsage() {
  console.log("usage: node scripts/smoke-vsix.mjs --vsix <path>");
}

function fail(message) {
  throw new Error(message);
}

function isMainModule() {
  return (
    process.argv[1] !== undefined &&
    path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
  );
}
