import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { digestJson } from "../stable-json.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");

export function collectHarnessInputFiles() {
  const files = [
    path.join(nodeRoot, "candidate-builds.json"),
    path.join(nodeRoot, "package-lock.json"),
    path.join(nodeRoot, "package-surfaces.json"),
    path.join(nodeRoot, "package.json"),
    path.join(repositoryRoot, "LICENSE-APACHE"),
    path.join(repositoryRoot, "LICENSE-MIT"),
    path.join(repositoryRoot, "THIRD_PARTY_NOTICES.md"),
    ...walkFiles(path.join(nodeRoot, "benchmark")),
    ...walkFiles(path.join(nodeRoot, "packages")),
    ...walkFiles(path.join(nodeRoot, "scripts")),
    ...walkFiles(path.join(nodeRoot, "src")),
    ...walkFiles(path.join(repositoryRoot, "THIRD_PARTY_LICENSES")),
  ];
  return [...new Set(files.map((file) => path.resolve(file)))]
    .filter((file) => existsSync(file))
    .sort((left, right) => left.localeCompare(right));
}

export function computeHarnessDigest() {
  return digestJson(
    collectHarnessInputFiles().map((file) => ({
      path: path.relative(repositoryRoot, file).split(path.sep).join("/"),
      sha256: digestFile(file),
    })),
  );
}

function walkFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute));
    else if (entry.isFile()) files.push(absolute);
  }
  return files;
}

function digestFile(file) {
  return `sha256:${createHash("sha256").update(readFileSync(file)).digest("hex")}`;
}
