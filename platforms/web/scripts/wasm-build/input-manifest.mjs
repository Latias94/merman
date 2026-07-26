import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";

export const WASM_INPUT_MANIFEST_SCHEMA_VERSION = 1;
export const WASM_INPUT_MANIFEST_NAME = "merman_wasm_inputs.json";

const ROOT_FILE_INPUTS = [
  ".cargo/config",
  ".cargo/config.toml",
  "Cargo.lock",
  "Cargo.toml",
  "rust-toolchain",
  "rust-toolchain.toml",
  "abi/text-measurement-v1.json",
  "capabilities/artifact-profiles-v1.json",
  "capabilities/feature-surface-v1.json",
  "capabilities/generated/capability_surface.rs",
  "platforms/web/scripts/build-wasm.mjs",
  "platforms/web/web-surface-descriptor.schema.json",
  "platforms/web/web-surface-descriptor.json",
];
const OWNED_BUILD_MODULE_ROOT = "platforms/web/scripts/wasm-build";
const REQUIRED_ARTIFACT_FILES = Object.freeze([
  "merman_wasm.d.ts",
  "merman_wasm.js",
  "merman_wasm_bg.wasm",
  "merman_wasm_bg.wasm.d.ts",
  "merman_wasm_artifact_profile.json",
  "package.json",
]);
export const WASM_ARTIFACT_FILE_NAMES = Object.freeze([
  ...REQUIRED_ARTIFACT_FILES,
  WASM_INPUT_MANIFEST_NAME,
]);
const ARTIFACT_FILES = new Set(REQUIRED_ARTIFACT_FILES);

export function buildWasmInputManifest({
  metadata,
  outputRoot,
  preset,
  repoRoot,
  toolVersions,
}) {
  const config = normalizedBuildConfig(preset);
  const inputs = collectWasmInputEntries({ metadata, repoRoot });
  const artifacts = collectArtifactEntries(outputRoot);
  const sourceDigest = digestJson(inputs);
  const normalizedTools = normalizedToolVersions(toolVersions);
  const inputDigest = digestJson({
    config,
    source_digest: sourceDigest,
    tool_versions: normalizedTools,
  });

  return {
    schema_version: WASM_INPUT_MANIFEST_SCHEMA_VERSION,
    package: "merman-wasm",
    target: "wasm32-unknown-unknown",
    profile: "wasm-size",
    preset: config,
    tool_versions: normalizedTools,
    source_digest: sourceDigest,
    input_digest: inputDigest,
    inputs,
    artifacts,
  };
}

export function verifyWasmInputManifest({
  manifest,
  metadata,
  outputRoot,
  preset,
  repoRoot,
  toolVersions,
}) {
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return failure("WASM input manifest is missing or invalid.");
  }
  if (manifest.schema_version !== WASM_INPUT_MANIFEST_SCHEMA_VERSION) {
    return failure(
      `WASM input manifest schema is ${String(manifest.schema_version)}, expected ${WASM_INPUT_MANIFEST_SCHEMA_VERSION}.`,
    );
  }
  if (!isManifestShape(manifest)) {
    return failure("WASM input manifest has an invalid structure.");
  }

  const reasons = [];
  const expectedConfig = normalizedBuildConfig(preset);
  const expectedTools = normalizedToolVersions(toolVersions);
  if (stableJson(manifest.preset) !== stableJson(expectedConfig)) {
    reasons.push("WASM preset or feature configuration changed.");
  }
  if (stableJson(manifest.tool_versions) !== stableJson(expectedTools)) {
    reasons.push("WASM build tool versions changed.");
  }

  let currentInputs;
  let currentArtifacts;
  try {
    currentInputs = collectWasmInputEntries({ metadata, repoRoot });
    currentArtifacts = collectArtifactEntries(outputRoot);
  } catch (error) {
    reasons.push(error instanceof Error ? error.message : String(error));
    return { ok: false, reasons };
  }

  compareEntries("input", manifest.inputs, currentInputs, reasons);
  compareEntries("artifact", manifest.artifacts, currentArtifacts, reasons);

  const sourceDigest = digestJson(currentInputs);
  if (manifest.source_digest !== sourceDigest) {
    reasons.push("WASM source digest is inconsistent with the current inputs.");
  }
  const inputDigest = digestJson({
    config: expectedConfig,
    source_digest: sourceDigest,
    tool_versions: expectedTools,
  });
  if (manifest.input_digest !== inputDigest) {
    reasons.push("WASM input digest is inconsistent with the current build contract.");
  }

  return { ok: reasons.length === 0, reasons };
}

export function cargoMetadataForPreset({ preset, repoRoot }) {
  const config = normalizedBuildConfig(preset);
  const probeRoot = mkdtempSync(path.join(os.tmpdir(), "merman-wasm-metadata-"));
  try {
    mkdirSync(path.join(probeRoot, "src"));
    writeFileSync(path.join(probeRoot, "src", "lib.rs"), "");
    writeFileSync(
      path.join(probeRoot, "Cargo.toml"),
      isolatedProbeManifest(config, repoRoot),
    );
    const result = runCapture(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        "--offline",
        "--filter-platform",
        "wasm32-unknown-unknown",
        "--manifest-path",
        path.join(probeRoot, "Cargo.toml"),
      ],
      repoRoot,
    );
    try {
      return JSON.parse(result);
    } catch (error) {
      throw new Error(
        `cargo metadata returned invalid JSON: ${error instanceof Error ? error.message : String(error)}`,
      );
    }
  } finally {
    rmSync(probeRoot, { recursive: true, force: true });
  }
}

export function currentWasmBuildToolVersions(repoRoot) {
  return {
    cargo: runCapture("cargo", ["--version"], repoRoot),
    node: process.version,
    rustc: runCapture("rustc", ["--version", "--verbose"], repoRoot),
    wasm_pack: runCapture("wasm-pack", ["--version"], repoRoot),
  };
}

export function collectWasmInputEntries({ metadata, repoRoot }) {
  const files = new Set();
  for (const relative of ROOT_FILE_INPUTS) {
    const absolute = resolveRepositoryPath(repoRoot, relative);
    if (existsSync(absolute)) files.add(absolute);
  }

  const buildModuleRoot = resolveRepositoryPath(repoRoot, OWNED_BUILD_MODULE_ROOT);
  if (!existsSync(buildModuleRoot)) {
    throw new Error(`WASM build implementation directory is missing: ${OWNED_BUILD_MODULE_ROOT}`);
  }
  addTreeFiles(buildModuleRoot, files);

  for (const packageInfo of workspaceDependencyPackages(metadata, repoRoot)) {
    const manifest = path.resolve(packageInfo.manifest_path);
    files.add(manifest);
    const packageRoot = path.dirname(manifest);
    const rustSources = [];
    const buildScript = path.join(packageRoot, "build.rs");
    if (existsSync(buildScript)) {
      files.add(buildScript);
      rustSources.push(buildScript);
    }
    const sourceRoot = path.join(packageRoot, "src");
    assertProductionTargetsAreOwned(packageInfo, packageRoot, sourceRoot, buildScript);
    if (existsSync(sourceRoot)) {
      addTreeFiles(sourceRoot, files, (absolute) => {
        if (path.extname(absolute) === ".rs") rustSources.push(absolute);
      });
    }
    addEmbeddedRustInputs({ files, repoRoot, rustSources });
  }

  return [...files]
    .map((absolute) => fileEntry(repoRoot, absolute))
    .sort((left, right) => compareNames(left.path, right.path));
}

export function collectArtifactEntries(outputRoot) {
  if (!existsSync(outputRoot)) {
    throw new Error(`WASM artifact directory is missing: ${outputRoot}`);
  }
  for (const required of REQUIRED_ARTIFACT_FILES) {
    if (!existsSync(path.join(outputRoot, required))) {
      throw new Error(`WASM artifact is missing: ${required}`);
    }
  }

  const files = new Set();
  for (const entry of readdirSync(outputRoot, { withFileTypes: true })) {
    const absolute = path.join(outputRoot, entry.name);
    if (entry.isFile()) {
      if (entry.name === WASM_INPUT_MANIFEST_NAME) continue;
      if (!ARTIFACT_FILES.has(entry.name)) {
        throw new Error(`Unowned WASM artifact file: ${entry.name}`);
      }
      files.add(absolute);
    } else if (entry.isDirectory() && entry.name === "snippets") {
      addTreeFiles(absolute, files);
    } else {
      throw new Error(`Unowned WASM artifact entry: ${entry.name}`);
    }
  }
  return [...files]
    .map((absolute) => fileEntry(outputRoot, absolute))
    .sort((left, right) => compareNames(left.path, right.path));
}

function workspaceDependencyPackages(metadata, repoRoot) {
  if (!metadata || !Array.isArray(metadata.packages) || !metadata.resolve) {
    throw new Error("cargo metadata is missing packages or the resolve graph.");
  }
  const packagesById = new Map(metadata.packages.map((item) => [item.id, item]));
  const nodesById = new Map(
    (metadata.resolve.nodes ?? []).map((item) => [item.id, item]),
  );
  const rootId = metadata.resolve.root;
  if (!rootId) throw new Error("cargo metadata does not identify the isolated probe root.");

  const selected = new Set();
  const pending = [rootId];
  while (pending.length > 0) {
    const id = pending.pop();
    if (!id || selected.has(id)) continue;
    selected.add(id);
    const node = nodesById.get(id);
    if (!node || !Array.isArray(node.deps)) {
      throw new Error(`cargo metadata resolve node is missing dependency kinds: ${id}`);
    }
    for (const dependency of node.deps) {
      if (
        dependency.dep_kinds?.some(
          (kind) => kind.kind === null || kind.kind === "build",
        )
      ) {
        pending.push(dependency.pkg);
      }
    }
  }

  const canonicalRepo = path.resolve(repoRoot);
  return [...selected]
    .map((id) => packagesById.get(id))
    .filter((item) => {
      if (!item || item.source !== null) return false;
      const manifest = path.resolve(item.manifest_path);
      return isWithin(canonicalRepo, manifest);
    });
}

function assertProductionTargetsAreOwned(packageInfo, packageRoot, sourceRoot, buildScript) {
  for (const target of packageInfo.targets ?? []) {
    const kinds = new Set(target.kind ?? []);
    if (!["lib", "proc-macro", "custom-build"].some((kind) => kinds.has(kind))) {
      continue;
    }
    const targetSource = path.resolve(target.src_path);
    if (
      !isWithin(sourceRoot, targetSource) &&
      targetSource !== path.resolve(buildScript)
    ) {
      throw new Error(
        `WASM dependency production target escapes its owned src/build.rs roots: ${normalizePath(path.relative(packageRoot, targetSource))}`,
      );
    }
  }
}

function addTreeFiles(root, files, onFile = () => {}) {
  const stat = lstatSync(root);
  if (stat.isSymbolicLink()) {
    throw new Error(`WASM input tree contains a symbolic link: ${root}`);
  }
  if (stat.isFile()) {
    files.add(root);
    onFile(root);
    return;
  }
  if (!stat.isDirectory()) return;
  for (const entry of readdirSync(root, { withFileTypes: true }).sort((left, right) =>
    compareNames(left.name, right.name),
  )) {
    addTreeFiles(path.join(root, entry.name), files, onFile);
  }
}

function addEmbeddedRustInputs({ files, repoRoot, rustSources }) {
  for (const sourcePath of rustSources) {
    const source = readFileSync(sourcePath, "utf8");
    for (const embedded of embeddedRustInputs(source, sourcePath, repoRoot)) {
      files.add(embedded);
    }
  }
}

function embeddedRustInputs(source, sourcePath, repoRoot) {
  const inputs = [];
  let cursor = 0;
  while (cursor < source.length) {
    if (source.startsWith("//", cursor)) {
      cursor = skipLineComment(source, cursor);
      continue;
    }
    if (source.startsWith("/*", cursor)) {
      cursor = skipBlockComment(source, cursor);
      continue;
    }

    const rawString = parseRawString(source, cursor);
    if (rawString) {
      cursor = rawString.end;
      continue;
    }
    if (source[cursor] === '"') {
      cursor = scanQuotedString(source, cursor);
      continue;
    }
    if (source[cursor] === "'") {
      cursor = scanCharacterLiteral(source, cursor);
      continue;
    }
    if (!isIdentifierStart(source[cursor])) {
      cursor += 1;
      continue;
    }

    const identifierStart = cursor;
    cursor = scanIdentifier(source, cursor);
    const macroName = source.slice(identifierStart, cursor);
    if (macroName !== "include_str" && macroName !== "include_bytes") {
      continue;
    }

    let argument = skipRustTrivia(source, cursor);
    if (source[argument] !== "!") continue;
    argument = skipRustTrivia(source, argument + 1);
    if (source[argument] === "=") continue;
    const closingDelimiter = {
      "(": ")",
      "[": "]",
      "{": "}",
    }[source[argument]];
    if (!closingDelimiter) {
      throw unresolvedRustInput(macroName, source, sourcePath, repoRoot, argument);
    }
    argument = skipRustTrivia(source, argument + 1);

    const literal = parseRustStringLiteral(source, argument);
    if (!literal) {
      throw unresolvedRustInput(macroName, source, sourcePath, repoRoot, argument);
    }
    let end = skipRustTrivia(source, literal.end);
    if (source[end] === ",") end = skipRustTrivia(source, end + 1);
    if (source[end] !== closingDelimiter) {
      throw unresolvedRustInput(macroName, source, sourcePath, repoRoot, end);
    }

    const absolute = path.resolve(path.dirname(sourcePath), literal.value);
    if (!isWithin(path.resolve(repoRoot), absolute)) {
      throw new Error(
        `WASM ${macroName}! input escapes the repository: ${normalizePath(path.relative(repoRoot, absolute))}`,
      );
    }
    if (!existsSync(absolute)) {
      throw new Error(
        `WASM ${macroName}! input is missing: ${normalizePath(path.relative(repoRoot, absolute))}`,
      );
    }
    inputs.push(absolute);
    cursor = end + 1;
  }
  return inputs;
}

function parseRustStringLiteral(source, cursor) {
  const raw = parseRawString(source, cursor, { requireBarePrefix: true });
  if (raw) return raw;
  if (source[cursor] !== '"') return null;

  let value = "";
  let index = cursor + 1;
  while (index < source.length) {
    const character = source[index];
    if (character === '"') return { end: index + 1, value };
    if (character === "\n" || character === "\r") {
      throw new Error("Rust string literal contains an unescaped newline.");
    }
    if (character !== "\\") {
      value += character;
      index += 1;
      continue;
    }

    const escaped = decodeRustStringEscape(source, index);
    value += escaped.value;
    index = escaped.end;
  }
  throw new Error("Rust string literal is unterminated.");
}

function parseRawString(source, cursor, { requireBarePrefix = false } = {}) {
  const prefixes = requireBarePrefix ? ["r"] : ["br", "cr", "r"];
  const prefix = prefixes.find((candidate) => source.startsWith(candidate, cursor));
  if (!prefix || isIdentifierContinue(source[cursor - 1])) return null;

  let quote = cursor + prefix.length;
  let hashes = 0;
  while (source[quote] === "#") {
    hashes += 1;
    quote += 1;
  }
  if (source[quote] !== '"') return null;

  const closing = `"${"#".repeat(hashes)}`;
  const endQuote = source.indexOf(closing, quote + 1);
  if (endQuote === -1) throw new Error("Rust raw string literal is unterminated.");
  return {
    end: endQuote + closing.length,
    value: source.slice(quote + 1, endQuote),
  };
}

function decodeRustStringEscape(source, slash) {
  const escaped = source[slash + 1];
  const simple = {
    0: "\0",
    n: "\n",
    r: "\r",
    t: "\t",
    '"': '"',
    "'": "'",
    "\\": "\\",
  };
  if (Object.hasOwn(simple, escaped)) {
    return { end: slash + 2, value: simple[escaped] };
  }
  if (escaped === "x") {
    const digits = source.slice(slash + 2, slash + 4);
    if (!/^[0-9a-fA-F]{2}$/.test(digits)) {
      throw new Error("Rust string literal has an invalid hexadecimal escape.");
    }
    return { end: slash + 4, value: String.fromCharCode(Number.parseInt(digits, 16)) };
  }
  if (escaped === "u" && source[slash + 2] === "{") {
    const close = source.indexOf("}", slash + 3);
    const digits = close === -1 ? "" : source.slice(slash + 3, close).replaceAll("_", "");
    if (!/^[0-9a-fA-F]{1,6}$/.test(digits)) {
      throw new Error("Rust string literal has an invalid Unicode escape.");
    }
    return {
      end: close + 1,
      value: String.fromCodePoint(Number.parseInt(digits, 16)),
    };
  }
  if (escaped === "\n" || (escaped === "\r" && source[slash + 2] === "\n")) {
    let end = slash + (escaped === "\r" ? 3 : 2);
    while (source[end] === " " || source[end] === "\t" || source[end] === "\n") end += 1;
    return { end, value: "" };
  }
  throw new Error(`Rust string literal has an unsupported escape: \\${escaped ?? ""}`);
}

function scanQuotedString(source, quote) {
  let cursor = quote + 1;
  while (cursor < source.length) {
    if (source[cursor] === "\\") {
      cursor += 2;
    } else if (source[cursor] === '"') {
      return cursor + 1;
    } else {
      cursor += 1;
    }
  }
  return source.length;
}

function scanCharacterLiteral(source, quote) {
  let cursor = quote + 1;
  if (source[cursor] === "\\") {
    cursor += 2;
  } else {
    cursor += 1;
  }
  return source[cursor] === "'" ? cursor + 1 : quote + 1;
}

function skipRustTrivia(source, cursor) {
  while (cursor < source.length) {
    if (/\s/.test(source[cursor])) {
      cursor += 1;
    } else if (source.startsWith("//", cursor)) {
      cursor = skipLineComment(source, cursor);
    } else if (source.startsWith("/*", cursor)) {
      cursor = skipBlockComment(source, cursor);
    } else {
      break;
    }
  }
  return cursor;
}

function skipLineComment(source, cursor) {
  const newline = source.indexOf("\n", cursor + 2);
  return newline === -1 ? source.length : newline + 1;
}

function skipBlockComment(source, cursor) {
  let depth = 1;
  cursor += 2;
  while (cursor < source.length && depth > 0) {
    if (source.startsWith("/*", cursor)) {
      depth += 1;
      cursor += 2;
    } else if (source.startsWith("*/", cursor)) {
      depth -= 1;
      cursor += 2;
    } else {
      cursor += 1;
    }
  }
  return cursor;
}

function scanIdentifier(source, cursor) {
  cursor += 1;
  while (isIdentifierContinue(source[cursor])) cursor += 1;
  return cursor;
}

function isIdentifierStart(character) {
  return Boolean(character && /[A-Za-z_]/.test(character));
}

function isIdentifierContinue(character) {
  return Boolean(character && /[A-Za-z0-9_]/.test(character));
}

function unresolvedRustInput(macroName, source, sourcePath, repoRoot, cursor) {
  const line = source.slice(0, cursor).split("\n").length;
  const relative = normalizePath(path.relative(repoRoot, sourcePath));
  return new Error(
    `Cannot resolve ${macroName}! input in ${relative}:${line}; use a string literal so the WASM input manifest stays fail-closed.`,
  );
}

function fileEntry(root, absolute) {
  const canonicalRoot = path.resolve(root);
  const canonicalFile = path.resolve(absolute);
  if (!isWithin(canonicalRoot, canonicalFile)) {
    throw new Error(`WASM input escapes its repository root: ${canonicalFile}`);
  }
  const stat = lstatSync(canonicalFile);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`WASM input is not a regular file: ${canonicalFile}`);
  }
  if (!isWithin(realpathSync(canonicalRoot), realpathSync(canonicalFile))) {
    throw new Error(`WASM input resolves outside its repository root: ${canonicalFile}`);
  }
  return {
    path: normalizePath(path.relative(canonicalRoot, canonicalFile)),
    sha256: sha256(readFileSync(canonicalFile)),
  };
}

function normalizedBuildConfig(preset) {
  if (!preset || typeof preset.name !== "string") {
    throw new Error("WASM preset descriptor is invalid.");
  }
  if (
    !Array.isArray(preset.runtime_capability_ids) ||
    !preset.runtime_capability_ids.every((item) => typeof item === "string")
  ) {
    throw new Error("WASM preset runtime capability IDs are invalid.");
  }
  if (
    !Array.isArray(preset.runtime_output_ids) ||
    !preset.runtime_output_ids.every((item) => typeof item === "string")
  ) {
    throw new Error("WASM preset runtime output IDs are invalid.");
  }
  if (typeof preset.default_features !== "boolean") {
    throw new Error("WASM preset default_features must be boolean.");
  }
  if (!Array.isArray(preset.features) || !preset.features.every((item) => typeof item === "string")) {
    throw new Error("WASM preset features must be strings.");
  }
  return {
    name: preset.name,
    surface: preset.surface,
    default_features: preset.default_features,
    features: [...preset.features].sort(compareNames),
    runtime_capability_ids: [...preset.runtime_capability_ids].sort(compareNames),
    runtime_output_ids: [...preset.runtime_output_ids].sort(compareNames),
  };
}

function normalizedToolVersions(toolVersions) {
  if (!toolVersions || typeof toolVersions !== "object") {
    throw new Error("WASM build tool versions are missing.");
  }
  const output = {};
  for (const key of ["cargo", "node", "rustc", "wasm_pack"]) {
    const value = toolVersions[key];
    if (typeof value !== "string" || value.length === 0) {
      throw new Error(`WASM build tool version is missing: ${key}`);
    }
    output[key] = value;
  }
  return output;
}

function compareEntries(kind, expected, actual, reasons) {
  const expectedByPath = new Map(expected.map((entry) => [entry.path, entry.sha256]));
  const actualByPath = new Map(actual.map((entry) => [entry.path, entry.sha256]));
  for (const [entryPath, hash] of actualByPath) {
    if (!expectedByPath.has(entryPath)) {
      reasons.push(`WASM ${kind} added: ${entryPath}`);
    } else if (expectedByPath.get(entryPath) !== hash) {
      reasons.push(`WASM ${kind} changed: ${entryPath}`);
    }
  }
  for (const entryPath of expectedByPath.keys()) {
    if (!actualByPath.has(entryPath)) {
      reasons.push(`WASM ${kind} removed: ${entryPath}`);
    }
  }
}

function isManifestShape(manifest) {
  return (
    manifest.package === "merman-wasm" &&
    manifest.target === "wasm32-unknown-unknown" &&
    manifest.profile === "wasm-size" &&
    isSha256(manifest.source_digest) &&
    isSha256(manifest.input_digest) &&
    isUniqueFileEntryArray(manifest.inputs) &&
    isUniqueFileEntryArray(manifest.artifacts) &&
    manifest.preset &&
    typeof manifest.preset === "object" &&
    isToolVersions(manifest.tool_versions)
  );
}

function isUniqueFileEntryArray(value) {
  if (!Array.isArray(value) || !value.every(isFileEntry)) return false;
  return new Set(value.map((entry) => entry.path)).size === value.length;
}

function isToolVersions(value) {
  return (
    value &&
    typeof value === "object" &&
    ["cargo", "node", "rustc", "wasm_pack"].every(
      (key) => typeof value[key] === "string" && value[key].length > 0,
    )
  );
}

function isFileEntry(entry) {
  return (
    entry &&
    typeof entry === "object" &&
    typeof entry.path === "string" &&
    entry.path.length > 0 &&
    isSha256(entry.sha256)
  );
}

function isolatedProbeManifest(config, repoRoot) {
  const features = config.features.map((feature) => JSON.stringify(feature)).join(", ");
  return [
    "[package]",
    'name = "merman-wasm-freshness-probe"',
    'version = "0.0.0"',
    'edition = "2024"',
    "publish = false",
    "",
    "[dependencies]",
    `merman-wasm = { path = ${JSON.stringify(path.join(repoRoot, "crates", "merman-wasm"))}, default-features = ${config.default_features}, features = [${features}] }`,
    "",
    "[workspace]",
    'resolver = "2"',
    "",
  ].join("\n");
}

function runCapture(command, args, cwd) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.error) {
    throw new Error(`Failed to run ${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed: ${(result.stderr || result.stdout).trim()}`,
    );
  }
  return result.stdout.trim();
}

function failure(reason) {
  return { ok: false, reasons: [reason] };
}

function digestJson(value) {
  return sha256(Buffer.from(stableJson(value)));
}

function stableJson(value) {
  return JSON.stringify(value);
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function isSha256(value) {
  return typeof value === "string" && /^[0-9a-f]{64}$/.test(value);
}

function isWithin(root, candidate) {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
}

function resolveRepositoryPath(repoRoot, relative) {
  return path.join(repoRoot, ...relative.split("/"));
}

function normalizePath(value) {
  return value.split(path.sep).join("/");
}

function compareNames(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}
