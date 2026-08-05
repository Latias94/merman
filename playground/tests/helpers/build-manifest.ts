import { readFileSync } from "node:fs";
import path from "node:path";
import { OPTIONAL_FEATURE_SOURCES } from "../../scripts/playground-build-policy.mjs";
import {
  manifestChunk,
  manifestKeysForSource,
  parseViteManifest,
} from "../../scripts/vite-manifest-graph.mjs";

type OptionalFeature = keyof typeof OPTIONAL_FEATURE_SOURCES;

const playgroundRoot = path.resolve(import.meta.dirname, "../..");
const graph = loadManifestGraph();

export function optionalFeatureOutput(feature: OptionalFeature): string {
  const source = OPTIONAL_FEATURE_SOURCES[feature];
  const matches = manifestKeysForSource(graph, source);
  if (matches.length !== 1) {
    throw new Error(
      `Expected one Vite output for ${source}; found ${matches.length}.`,
    );
  }
  return manifestChunk(graph, matches[0]).file;
}

function loadManifestGraph() {
  return parseViteManifest(
    JSON.parse(
    readFileSync(
      path.join(playgroundRoot, "dist", ".vite", "manifest.json"),
      "utf8",
    ),
    ),
  );
}
