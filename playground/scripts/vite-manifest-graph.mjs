export class ViteManifestContractError extends Error {
  constructor(message) {
    super(message);
    this.name = "ViteManifestContractError";
  }
}

export function parseViteManifest(value) {
  if (!isRecord(value)) {
    throw new ViteManifestContractError("Vite manifest must be an object.");
  }
  const chunks = {};
  const outputOwners = new Map();
  for (const [key, rawChunk] of Object.entries(value)) {
    if (!key || !isRecord(rawChunk)) {
      throw new ViteManifestContractError(`Invalid Vite manifest chunk ${key}.`);
    }
    const file = emittedPath(rawChunk.file, `${key} file`);
    const previousOwner = outputOwners.get(file);
    if (previousOwner) {
      throw new ViteManifestContractError(
        `Vite output ${file} is owned by both ${previousOwner} and ${key}.`,
      );
    }
    outputOwners.set(file, key);
    chunks[key] = Object.freeze({
      ...rawChunk,
      file,
      ...(rawChunk.src === undefined
        ? {}
        : { src: logicalId(rawChunk.src, `${key} source`) }),
      imports: stringList(rawChunk.imports, `${key} imports`),
      dynamicImports: stringList(
        rawChunk.dynamicImports,
        `${key} dynamic imports`,
      ),
      assets: pathList(rawChunk.assets, `${key} assets`),
      css: pathList(rawChunk.css, `${key} CSS`),
    });
  }
  for (const [key, chunk] of Object.entries(chunks)) {
    for (const dependency of [...chunk.imports, ...chunk.dynamicImports]) {
      if (!Object.hasOwn(chunks, dependency)) {
        throw new ViteManifestContractError(
          `Vite manifest chunk ${key} references unknown chunk ${dependency}.`,
        );
      }
    }
  }
  return Object.freeze({ chunks: Object.freeze(chunks) });
}

export function manifestSource(graph, key) {
  const chunk = manifestChunk(graph, key);
  return String(chunk.src ?? key).replaceAll("\\", "/");
}

export function manifestKeysForSource(graph, source) {
  const expected = logicalId(source, "manifest source");
  return Object.keys(graph.chunks).filter(
    (key) => manifestSource(graph, key) === expected,
  );
}

export function requireUniqueManifestSource(graph, source, predicate = () => true) {
  const matches = manifestKeysForSource(graph, source).filter((key) =>
    predicate(graph.chunks[key]),
  );
  if (matches.length !== 1) {
    throw new ViteManifestContractError(
      `Expected one Vite manifest node for ${source}; found ${matches.length}.`,
    );
  }
  return matches[0];
}

export function collectManifestClosure(graph, roots, mode = "static") {
  if (mode !== "static" && mode !== "reachable") {
    throw new ViteManifestContractError(`Unknown manifest closure mode ${mode}.`);
  }
  const visited = new Set();
  const pending = [...roots];
  while (pending.length > 0) {
    const key = pending.pop();
    if (visited.has(key)) continue;
    const chunk = manifestChunk(graph, key);
    visited.add(key);
    pending.push(...chunk.imports);
    if (mode === "reachable") pending.push(...chunk.dynamicImports);
  }
  return visited;
}

export function emittedFiles(graph, keys) {
  return new Set([...keys].map((key) => manifestChunk(graph, key).file));
}

export function ownersOfAsset(graph, assetFile) {
  const asset = emittedPath(assetFile, "asset file");
  return Object.entries(graph.chunks)
    .filter(([, chunk]) => chunk.assets.includes(asset))
    .map(([key]) => key);
}

export function manifestChunk(graph, key) {
  const chunk = graph.chunks[key];
  if (!chunk) {
    throw new ViteManifestContractError(`Unknown Vite manifest node ${String(key)}.`);
  }
  return chunk;
}

function stringList(value, label) {
  if (value === undefined) return Object.freeze([]);
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new ViteManifestContractError(`${label} must be a string array.`);
  }
  if (new Set(value).size !== value.length) {
    throw new ViteManifestContractError(`${label} contains duplicates.`);
  }
  return Object.freeze([...value]);
}

function pathList(value, label) {
  return Object.freeze(stringList(value, label).map((item) => emittedPath(item, label)));
}

function logicalId(value, label) {
  if (typeof value !== "string" || value.length === 0 || value.includes("\0")) {
    throw new ViteManifestContractError(`Invalid ${label}.`);
  }
  return value.replaceAll("\\", "/");
}

function emittedPath(value, label) {
  const normalized = logicalId(value, label);
  if (
    normalized.startsWith("/") ||
    /^[A-Za-z]:\//u.test(normalized) ||
    normalized.split("/").some((part) => part === "" || part === "." || part === "..")
  ) {
    throw new ViteManifestContractError(`Invalid ${label}: ${normalized}.`);
  }
  return normalized;
}

function isRecord(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
