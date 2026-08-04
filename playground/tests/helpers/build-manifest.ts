import { readFileSync } from "node:fs";
import path from "node:path";
import type { Manifest } from "vite";

import { OPTIONAL_FEATURE_SOURCES } from "../../scripts/optional-feature-manifest.mjs";

type OptionalFeature = keyof typeof OPTIONAL_FEATURE_SOURCES;

const playgroundRoot = path.resolve(import.meta.dirname, "../..");
const outputsBySource = indexManifestOutputs();

export function optionalFeatureOutput(feature: OptionalFeature): string {
  const source = OPTIONAL_FEATURE_SOURCES[feature];
  const matches = outputsBySource.get(source) ?? [];
  if (matches.length !== 1) {
    throw new Error(
      `Expected one Vite output for ${source}; found ${matches.length}.`,
    );
  }
  return matches[0];
}

function indexManifestOutputs(): ReadonlyMap<string, readonly string[]> {
  const manifest = JSON.parse(
    readFileSync(
      path.join(playgroundRoot, "dist", ".vite", "manifest.json"),
      "utf8",
    ),
  ) as Readonly<Manifest>;
  const index = new Map<string, string[]>();
  for (const [key, chunk] of Object.entries(manifest)) {
    const source = (chunk.src ?? key).replaceAll("\\", "/");
    const outputs = index.get(source) ?? [];
    outputs.push(chunk.file);
    index.set(source, outputs);
  }
  return index;
}
