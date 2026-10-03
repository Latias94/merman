#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

import { assertSuccessfulNpmSpawn, spawnNpmSync } from "./npm-command.mjs";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const readText = (file) => fs.readFileSync(file, "utf8").replaceAll("\r\n", "\n").trim();
const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));

/** npm owns dependency resolution, including peers, links and optional dependencies. */
export function collectProductionLicenses(packageRoot, replacements = {}) {
  const result = spawnNpmSync(["ls", "--omit=dev", "--parseable", "--all"], {
    cwd: packageRoot,
    encoding: "utf8",
    maxBuffer: 8 * 1024 * 1024,
  });
  assertSuccessfulNpmSpawn(result, "npm production dependency inventory");
  const root = fs.realpathSync(packageRoot);
  const manifest = readJson(path.join(root, "package.json"));
  const groups = new Map();
  for (const directory of new Set(result.stdout.trim().split(/\r?\n/).filter(Boolean))) {
    if (fs.realpathSync(directory) === root) continue;
    const dependency = readJson(path.join(directory, "package.json"));
    // Platform canvas binaries are host tooling, not part of the browser payload.
    if (manifest.name === "merman-playground" &&
        /^@napi-rs\/canvas-(?:android|darwin|linux|win32)-/.test(dependency.name)) continue;
    const identity = `${dependency.name}@${dependency.version}`;
    const content = readLicenseContent(directory, dependency, replacements[dependency.name]);
    const names = groups.get(content) ?? new Set();
    names.add(identity);
    groups.set(content, names);
  }
  return [...groups].map(([content, names]) => ({ content, names: [...names].sort() }))
    .sort((left, right) => left.names[0] < right.names[0] ? -1 : left.names[0] > right.names[0] ? 1 : 0);
}

export function readLicenseContent(directory, manifest, replacement) {
  const files = fs.readdirSync(directory).filter((name) =>
    fs.statSync(path.join(directory, name)).isFile()).sort();
  const licenses = files.filter((name) => /^(?:license|licence|copying)(?:[-.]|$)/i.test(name) &&
    !/\.(?:[cm]?[jt]s|sh|ps1)$/i.test(name));
  if (!replacement && licenses.length > 1) {
    throw new Error(`Ambiguous license files for ${manifest.name}; add a canonical replacement.`);
  }
  // Preserve the existing explicit choices for dual-licensed or assembled packages.
  let content = replacement ? readText(replacement)
    : licenses.length === 1 ? readText(path.join(directory, licenses[0])) : null;
  if (content === null) {
    // Some published packages contain only an SPDX declaration. Do not invent a license body.
    if (typeof manifest.license !== "string" || !manifest.license.trim() ||
        /^SEE LICENSE IN /i.test(manifest.license)) {
      throw new Error(`Missing license text for ${manifest.name}; add a canonical replacement.`);
    }
    content = manifest.license.trim();
  }
  if (!content) throw new Error(`Empty license text for ${manifest.name}.`);
  const notices = files.filter((name) => /^notice(?:[-.]|$)/i.test(name))
    .map((name) => readText(path.join(directory, name))).filter(Boolean);
  return notices.length ? `${content}\n\nWith the following notices:\n\n${notices.join("\n")}` : content;
}

export function formatLicenseGroups(groups) {
  return groups.map(({ names, content }) => {
    const plural = names.length > 1;
    return [
      `The following npm package${plural ? "s" : ""} may be included in this product:`,
      "", ...names.map((name) => ` - ${name}`), "",
      plural ? "These packages each contain the following license:"
        : "This package contains the following license:",
      "", content,
    ].join("\n");
  }).join("\n\n-----------\n\n");
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const packageRoot = path.resolve(repositoryRoot, options.packageRoot);
  const output = path.resolve(repositoryRoot, options.output);
  const packageLock = path.join(packageRoot, "package-lock.json");
  const manifest = readJson(path.join(packageRoot, "package.json"));
  const replacements = Object.fromEntries(Object.entries({
    "@mermanjs/web": "platforms/web/LICENSE",
    cytoscape: "THIRD_PARTY_LICENSES/cytoscape/LICENSE",
    dompurify: "THIRD_PARTY_LICENSES/dompurify/LICENSE",
  }).map(([name, file]) => [name, path.join(repositoryRoot, file)]));
  const groups = collectProductionLicenses(packageRoot, replacements);
  const lockDigest = crypto.createHash("sha256").update(fs.readFileSync(packageLock)).digest("hex");
  const generated = [
    "Merman npm production dependency licenses",
    `Package: ${manifest.name}`,
    "Generator: npm ls --omit=dev --parseable --all / Merman license report",
    `package-lock.json SHA-256: ${lockDigest}`,
    "", formatLicenseGroups(groups), "",
  ].join("\n");
  if (options.write) {
    fs.mkdirSync(path.dirname(output), { recursive: true });
    atomicWrite(output, generated);
  }
  if (!fs.statSync(output, { throwIfNoEntry: false })?.isFile() || fs.readFileSync(output, "utf8") !== generated) {
    throw new Error(`stale or missing npm license report: ${path.relative(repositoryRoot, output)}; run with --write`);
  }
  console.log(`npm dependency license report: ok (${options.packageRoot}, ${Buffer.byteLength(generated)} bytes)`);
}

function parseArgs(args) {
  const parsed = { packageRoot: null, output: null, check: false, write: false };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--package-root") parsed.packageRoot = args[++index];
    else if (argument === "--output") parsed.output = args[++index];
    else if (argument === "--check") parsed.check = true;
    else if (argument === "--write") parsed.write = true;
    else throw new Error(`unknown argument: ${argument}`);
  }
  if (!parsed.packageRoot || !parsed.output || parsed.check === parsed.write) {
    throw new Error("usage: generate-npm-license-report.mjs --package-root <dir> --output <file> (--check|--write)");
  }
  return parsed;
}

function atomicWrite(outputPath, contents) {
  const temporary = `${outputPath}.tmp-${process.pid}`;
  fs.writeFileSync(temporary, contents, "utf8");
  fs.renameSync(temporary, outputPath);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
