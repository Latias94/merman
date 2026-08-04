export const OPTIONAL_FEATURE_SOURCES = Object.freeze({
  benchmark: "src/components/BenchWorkbench.tsx",
  config: "src/components/ConfigEditorFeature.tsx",
  examples: "src/components/ExampleGallery.tsx",
});

export function inspectOptionalFeatureManifest(
  manifest,
  entrySource = "index.html",
) {
  const violations = [];
  const entryMatches = manifestKeysForSource(manifest, entrySource).filter(
    (key) => manifest[key]?.isEntry === true,
  );
  if (entryMatches.length !== 1) {
    violations.push(
      `Expected one entry for ${entrySource}; found ${entryMatches.length}.`,
    );
    return {
      entryKey: null,
      featureRoots: Object.freeze({}),
      initialReachableKeys: new Set(),
      initialStaticKeys: new Set(),
      violations,
    };
  }

  const [entryKey] = entryMatches;
  const initialStaticKeys = collectClosure(
    manifest,
    [entryKey],
    false,
    violations,
  );
  const initialReachableKeys = collectClosure(
    manifest,
    [entryKey],
    true,
    violations,
  );
  const initialStaticFiles = new Set(
    [...initialStaticKeys]
      .map((key) => manifestFile(manifest, key, violations))
      .filter((file) => file !== null),
  );
  const featureRoots = {};

  for (const [feature, source] of Object.entries(OPTIONAL_FEATURE_SOURCES)) {
    const matches = manifestKeysForSource(manifest, source);
    if (matches.length !== 1) {
      violations.push(
        `Expected one ${feature} activation root for ${source}; found ${matches.length}.`,
      );
      continue;
    }
    const [root] = matches;
    featureRoots[feature] = root;
    const rootFile = manifestFile(manifest, root, violations);
    if (
      initialStaticKeys.has(root) ||
      (rootFile !== null && initialStaticFiles.has(rootFile))
    ) {
      violations.push(`${feature} is present in the initial static closure.`);
    }
    if (!initialReachableKeys.has(root)) {
      violations.push(`${feature} is not dynamically reachable from ${entrySource}.`);
    }
  }

  return {
    entryKey,
    featureRoots: Object.freeze(featureRoots),
    initialReachableKeys,
    initialStaticKeys,
    violations,
  };
}

function manifestKeysForSource(manifest, expectedSource) {
  return Object.entries(manifest)
    .filter(([key, chunk]) => manifestSource(key, chunk) === expectedSource)
    .map(([key]) => key);
}

function manifestSource(key, chunk) {
  return String(chunk?.src ?? key).replaceAll("\\", "/");
}

function manifestFile(manifest, key, violations) {
  const file = manifest[key]?.file;
  if (typeof file !== "string" || file.length === 0) {
    violations.push(`Manifest chunk ${String(key)} has no emitted file.`);
    return null;
  }
  return file.replaceAll("\\", "/");
}

function collectClosure(manifest, roots, includeDynamic, violations) {
  const visited = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (visited.has(key)) continue;
    const chunk = manifest[key];
    if (!chunk || typeof chunk !== "object") {
      violations.push(`Manifest references unknown chunk ${String(key)}.`);
      continue;
    }
    visited.add(key);
    pending.push(...(chunk.imports ?? []));
    if (includeDynamic) pending.push(...(chunk.dynamicImports ?? []));
  }
  return visited;
}
