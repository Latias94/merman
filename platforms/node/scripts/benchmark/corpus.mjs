import { readFileSync, readdirSync } from "node:fs";
import path from "node:path";

import { digestJson, stableJson } from "../stable-json.mjs";

const SHA256_DIGEST = /^sha256:[0-9a-f]{64}$/;

export function loadCorpus(manifestPath) {
  const manifestFile = path.resolve(manifestPath);
  const manifest = JSON.parse(readFileSync(manifestFile, "utf8"));
  if (manifest.schema_version !== 2 || !Array.isArray(manifest.roots)) {
    throw new Error("Node benchmark corpus manifest schema_version must be 2.");
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
  const workloads = validateBenchmarkWorkloads(
    manifest.workloads,
    "Node benchmark workloads",
  );
  const corpusDigest = computeCorpusDigest(cases);
  return {
    cases,
    bindingOptions,
    operationOptions,
    workloads,
    corpusDigest,
    digest: computeInputDigest({
      corpusDigest,
      bindingOptions,
      operationOptions,
      workloads,
    }),
    manifestPath: manifestFile,
  };
}

export function computeCorpusDigest(cases) {
  if (!Array.isArray(cases) || cases.length === 0) {
    throw new Error("benchmark corpus digest requires at least one case.");
  }
  const normalizedCases = [...cases]
    .map((item) => {
      if (
        !item ||
        typeof item.path !== "string" ||
        item.path.length === 0 ||
        typeof item.source !== "string"
      ) {
        throw new Error("benchmark corpus cases require a path and source.");
      }
      return { path: item.path, source: item.source };
    })
    .sort((left, right) => left.path.localeCompare(right.path));
  if (new Set(normalizedCases.map((item) => item.path)).size !== normalizedCases.length) {
    throw new Error("benchmark corpus case paths must be unique.");
  }
  return digestJson(normalizedCases);
}

export function computeInputDigest({
  corpusDigest,
  bindingOptions,
  operationOptions,
  workloads,
}) {
  if (!SHA256_DIGEST.test(corpusDigest ?? "")) {
    throw new Error("benchmark input corpus digest must be a sha256 digest.");
  }
  assertObject(bindingOptions, "benchmark input binding options");
  assertObject(operationOptions, "benchmark input operation options");
  return digestJson({
    schema_version: 2,
    corpus_digest: corpusDigest,
    binding_options: bindingOptions,
    operation_options: operationOptions,
    workloads: validateBenchmarkWorkloads(workloads, "benchmark input workloads"),
  });
}

export function validateBenchmarkWorkloads(value, label) {
  assertObject(value, label);
  const ids = ["cold_svg", "concurrency_svg"];
  if (stableJson(Object.keys(value).sort()) !== stableJson(ids)) {
    throw new Error(`${label} must define exactly ${ids.join(" and ")}.`);
  }
  const normalized = {};
  const paths = new Set();
  for (const id of ids) {
    const workload = value[id];
    if (
      !workload ||
      typeof workload !== "object" ||
      Array.isArray(workload) ||
      workload.operation_id !== "svg" ||
      typeof workload.path !== "string" ||
      workload.path.length === 0 ||
      typeof workload.source !== "string" ||
      workload.source.length === 0 ||
      paths.has(workload.path)
    ) {
      throw new Error(`${label}.${id} must be a distinct explicit SVG source.`);
    }
    paths.add(workload.path);
    normalized[id] = {
      operation_id: workload.operation_id,
      path: workload.path,
      source: workload.source,
    };
  }
  return normalized;
}

function assertObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object.`);
  }
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
