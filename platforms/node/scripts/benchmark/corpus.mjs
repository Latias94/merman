import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import { computeInputDigest } from "./report-contract.mjs";

export function loadCorpus(manifestPath) {
  const manifestFile = path.resolve(manifestPath);
  const manifest = JSON.parse(readFileSync(manifestFile, "utf8"));
  if (manifest.schema_version !== 1 || !Array.isArray(manifest.roots)) {
    throw new Error("Node benchmark corpus manifest is invalid.");
  }
  const extensions = new Set(manifest.extensions ?? [".mmd"]);
  const cases = [];
  for (const relativeRoot of manifest.roots) {
    const root = path.resolve(path.dirname(manifestFile), relativeRoot);
    for (const file of walkFiles(root)) {
      if (!extensions.has(path.extname(file))) continue;
      cases.push({
        path: path.relative(root, file).split(path.sep).join("/"),
        source: readFileSync(file, "utf8"),
      });
    }
  }
  cases.sort((left, right) => left.path.localeCompare(right.path));
  if (cases.length === 0) throw new Error("Node benchmark corpus contains no input cases.");
  const bindingOptions = manifest.binding_options;
  const operationOptions = manifest.operation_options;
  return {
    cases,
    bindingOptions,
    operationOptions,
    digest: computeInputDigest({ cases, bindingOptions, operationOptions }),
    manifestPath: manifestFile,
  };
}

function walkFiles(root) {
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute));
    else if (entry.isFile()) files.push(absolute);
  }
  return files;
}
