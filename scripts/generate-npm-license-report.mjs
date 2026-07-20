#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const GENERATOR_VERSION = "4.2.1";
const options = parseArgs(process.argv.slice(2));
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const packageRoot = path.resolve(repositoryRoot, options.packageRoot);
const output = path.resolve(repositoryRoot, options.output);
const packageJson = path.join(packageRoot, "package.json");
const packageLock = path.join(packageRoot, "package-lock.json");

for (const required of [packageJson, packageLock]) {
  if (!fs.statSync(required, { throwIfNoEntry: false })?.isFile()) {
    fail(`missing npm license-report input: ${path.relative(repositoryRoot, required)}`);
  }
}

const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), "merman-npm-licenses-"));
const generatedPath = path.join(temporaryRoot, "licenses.txt");
try {
  const npm = process.platform === "win32" ? "npm.cmd" : "npm";
  const result = spawnSync(
    npm,
    [
      "exec",
      "--",
      "generate-license-file",
      "--input",
      "package.json",
      "--output",
      generatedPath,
      "--overwrite",
      "--eol",
      "lf",
      "--ci",
      "--no-spinner",
    ],
    { cwd: packageRoot, encoding: "utf8" },
  );
  if (result.error || result.status !== 0) {
    fail(
      `generate-license-file failed for ${options.packageRoot}: ${result.error?.message ?? result.stderr ?? result.stdout}`,
    );
  }

  const lockDigest = crypto
    .createHash("sha256")
    .update(fs.readFileSync(packageLock))
    .digest("hex");
  const generated = [
    "Merman npm production dependency licenses",
    `Package: ${JSON.parse(fs.readFileSync(packageJson, "utf8")).name}`,
    `Generator: generate-license-file ${GENERATOR_VERSION}`,
    `package-lock.json SHA-256: ${lockDigest}`,
    "",
    fs.readFileSync(generatedPath, "utf8").replaceAll("\r\n", "\n").trimEnd(),
    "",
  ].join("\n");

  if (options.write) {
    fs.mkdirSync(path.dirname(output), { recursive: true });
    atomicWrite(output, generated);
  }
  if (!fs.statSync(output, { throwIfNoEntry: false })?.isFile()) {
    fail(`missing npm license report: ${path.relative(repositoryRoot, output)}`);
  }
  if (fs.readFileSync(output, "utf8") !== generated) {
    fail(
      `stale npm license report: ${path.relative(repositoryRoot, output)}; run with --write`,
    );
  }
  console.log(
    `npm dependency license report: ok (${options.packageRoot}, ${Buffer.byteLength(generated)} bytes)`,
  );
} finally {
  fs.rmSync(temporaryRoot, { recursive: true, force: true });
}

function parseArgs(args) {
  const parsed = { packageRoot: null, output: null, check: false, write: false };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--package-root") parsed.packageRoot = args[++index];
    else if (argument === "--output") parsed.output = args[++index];
    else if (argument === "--check") parsed.check = true;
    else if (argument === "--write") parsed.write = true;
    else fail(`unknown argument: ${argument}`);
  }
  if (!parsed.packageRoot || !parsed.output || parsed.check === parsed.write) {
    fail("usage: generate-npm-license-report.mjs --package-root <dir> --output <file> (--check|--write)");
  }
  return parsed;
}

function atomicWrite(outputPath, contents) {
  const temporary = `${outputPath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, contents, "utf8");
  fs.renameSync(temporary, outputPath);
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
