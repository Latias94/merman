#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdir, readFile, readdir, realpath, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { Worker } from "node:worker_threads";

const scriptPath = fileURLToPath(import.meta.url);
const collectorRoot = path.dirname(scriptPath);
const workspaceRootDefault = path.resolve(collectorRoot, "../../..");
const scopesPathDefault = path.join(collectorRoot, "scopes.json");
const workerPath = path.join(collectorRoot, "worker.mjs");

const sha256 = (value) => createHash("sha256").update(value).digest("hex");

const fail = (message) => {
  throw new Error(message);
};

const normalizeRelativePath = (value, description) => {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${description} must be a non-empty string`);
  }
  if (
    value.includes("\\") ||
    value.includes(":") ||
    [...value].some((character) => character.codePointAt(0) < 0x20) ||
    path.posix.isAbsolute(value)
  ) {
    fail(`${description} must be a portable relative path: ${JSON.stringify(value)}`);
  }
  const normalized = path.posix.normalize(value);
  if (normalized !== value || normalized === "." || normalized.startsWith("../")) {
    fail(`${description} must not escape or normalize differently: ${JSON.stringify(value)}`);
  }
  return value;
};

const parseArgs = (argv) => {
  const args = {
    workspaceRoot: workspaceRootDefault,
    mermaidRoot: path.join(workspaceRootDefault, "repo-ref/mermaid"),
    scopesPath: scopesPathDefault,
    output: null,
    scope: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const flag = argv[index];
    const next = () => {
      index += 1;
      if (index >= argv.length) {
        fail(`${flag} requires a value`);
      }
      return argv[index];
    };
    switch (flag) {
      case "--workspace-root":
        args.workspaceRoot = path.resolve(next());
        break;
      case "--mermaid-root":
        args.mermaidRoot = path.resolve(next());
        break;
      case "--scopes":
        args.scopesPath = path.resolve(next());
        break;
      case "--scope":
        args.scope = next();
        break;
      case "--output":
        args.output = path.resolve(next());
        break;
      case "--help":
      case "-h":
        return { help: true };
      default:
        fail(`unknown argument ${JSON.stringify(flag)}`);
    }
  }
  if (args.scope === null || args.output === null) {
    fail("--scope and --output are required");
  }
  return args;
};

const readJson = async (file, description) => {
  let bytes;
  try {
    bytes = await readFile(file);
  } catch (error) {
    fail(`failed to read ${description} ${file}: ${error.message}`);
  }
  try {
    return { bytes, value: JSON.parse(bytes) };
  } catch (error) {
    fail(`failed to parse ${description} ${file}: ${error.message}`);
  }
};

const requireExactKeys = (value, keys, description) => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    fail(`${description} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${description} keys must be ${expected.join(", ")}; found ${actual.join(", ")}`);
  }
};

const validateScopeConfig = (config, scopeId) => {
  requireExactKeys(
    config,
    [
      "schemaVersion",
      "mermaidVersion",
      "mermaidSourceTag",
      "mermaidSourceCommit",
      "nodeVersion",
      "pnpmVersion",
      "esbuildVersion",
      "testConfig",
      "renderHelper",
      "scopes",
    ],
    "collector scope catalog"
  );
  if (config.schemaVersion !== 1) {
    fail(`collector scope catalog schemaVersion must be 1, found ${config.schemaVersion}`);
  }
  if (!/^[0-9a-f]{40}$/.test(config.mermaidSourceCommit)) {
    fail("collector scope catalog mermaidSourceCommit must be a canonical Git SHA-1");
  }
  for (const field of ["nodeVersion", "pnpmVersion", "esbuildVersion"]) {
    if (typeof config[field] !== "string" || !/^\d+\.\d+\.\d+$/.test(config[field])) {
      fail(`collector scope catalog ${field} must be an exact semantic version`);
    }
  }
  normalizeRelativePath(config.testConfig, "testConfig");
  normalizeRelativePath(config.renderHelper, "renderHelper");
  const scope = config.scopes?.[scopeId];
  if (scope === undefined) {
    fail(`unknown collector scope ${JSON.stringify(scopeId)}`);
  }
  requireExactKeys(
    scope,
    [
      "description",
      "manifest",
      "selectors",
      "capturedHelpers",
      "passiveHelperImports",
      "allowedRuntimeEffects",
      "expectedActiveCalls",
      "expectedSkippedRegistrations",
      "reviewedSkippedRegistrations",
      "reviewedRemovals",
      "supplementalFixtures",
      "timeoutMs",
    ],
    `collector scope ${scopeId}`
  );
  normalizeRelativePath(scope.manifest, `${scopeId} manifest`);
  for (const selector of scope.selectors) {
    requireExactKeys(selector, ["kind", "path"], `${scopeId} selector`);
    if (!new Set(["directory", "file"]).has(selector.kind)) {
      fail(`${scopeId} selector kind must be directory or file`);
    }
    normalizeRelativePath(selector.path, `${scopeId} selector path`);
  }
  for (const [name, values] of [
    ["capturedHelpers", scope.capturedHelpers],
    ["passiveHelperImports", scope.passiveHelperImports],
    ["allowedRuntimeEffects", scope.allowedRuntimeEffects],
  ]) {
    if (
      !Array.isArray(values) ||
      values.some((value) => typeof value !== "string" || value.length === 0) ||
      new Set(values).size !== values.length
    ) {
      fail(`${scopeId} ${name} must contain unique non-empty strings`);
    }
  }
  if (!Array.isArray(scope.reviewedSkippedRegistrations)) {
    fail(`${scopeId} reviewedSkippedRegistrations must be an array`);
  }
  for (const entry of scope.reviewedSkippedRegistrations) {
    requireExactKeys(entry, ["registration", "reason"], `${scopeId} reviewed skip`);
    if (
      typeof entry.registration !== "string" ||
      entry.registration.length === 0 ||
      typeof entry.reason !== "string" ||
      entry.reason.length === 0
    ) {
      fail(`${scopeId} reviewed skip fields must be non-empty strings`);
    }
  }
  if (!Array.isArray(scope.reviewedRemovals)) {
    fail(`${scopeId} reviewedRemovals must be an array`);
  }
  for (const entry of scope.reviewedRemovals) {
    requireExactKeys(
      entry,
      ["sourceSpec", "registration", "helperOrdinal", "reason"],
      `${scopeId} reviewed removal`
    );
    normalizeRelativePath(entry.sourceSpec, `${scopeId} reviewed removal sourceSpec`);
    if (
      typeof entry.registration !== "string" ||
      entry.registration.length === 0 ||
      !Number.isSafeInteger(entry.helperOrdinal) ||
      entry.helperOrdinal < 1 ||
      typeof entry.reason !== "string" ||
      entry.reason.length === 0
    ) {
      fail(`${scopeId} reviewed removal fields are invalid`);
    }
  }
  if (!Array.isArray(scope.supplementalFixtures)) {
    fail(`${scopeId} supplementalFixtures must be an array`);
  }
  for (const fixture of scope.supplementalFixtures) {
    normalizeRelativePath(fixture, `${scopeId} supplemental fixture`);
  }
  if (new Set(scope.supplementalFixtures).size !== scope.supplementalFixtures.length) {
    fail(`${scopeId} supplementalFixtures must not contain duplicates`);
  }
  if (!Number.isSafeInteger(scope.expectedActiveCalls) || scope.expectedActiveCalls < 1) {
    fail(`${scopeId} expectedActiveCalls must be a positive integer`);
  }
  if (
    !Number.isSafeInteger(scope.expectedSkippedRegistrations) ||
    scope.expectedSkippedRegistrations < 0
  ) {
    fail(`${scopeId} expectedSkippedRegistrations must be a non-negative integer`);
  }
  if (!Number.isSafeInteger(scope.timeoutMs) || scope.timeoutMs < 100) {
    fail(`${scopeId} timeoutMs must be at least 100`);
  }
  return scope;
};

const runText = (command, args, cwd) => {
  try {
    return execFileSync(command, args, {
      cwd,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }).trim();
  } catch (error) {
    const stderr = typeof error.stderr === "string" ? error.stderr.trim() : "";
    fail(`failed to run ${command} ${args.join(" ")} in ${cwd}: ${stderr || error.message}`);
  }
};

const assertInside = async (root, candidate, description) => {
  const [canonicalRoot, canonicalCandidate] = await Promise.all([realpath(root), realpath(candidate)]);
  const relative = path.relative(canonicalRoot, canonicalCandidate);
  if (relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))) {
    return canonicalCandidate;
  }
  fail(`${description} escapes ${root}: ${candidate} -> ${canonicalCandidate}`);
};

const enumerateFiles = async (root) => {
  const files = [];
  const visit = async (directory) => {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => left.name.localeCompare(right.name));
    for (const entry of entries) {
      const child = path.join(directory, entry.name);
      if (entry.isSymbolicLink()) {
        fail(`collector source tree contains a symbolic link: ${child}`);
      }
      if (entry.isDirectory()) {
        await visit(child);
      } else if (entry.isFile()) {
        files.push(child);
      }
    }
  };
  await visit(root);
  return files;
};

const configStubSource = (specifier) => {
  switch (specifier) {
    case "@applitools/eyes-cypress":
    case "cypress-split":
      return "export default (config) => config;";
    case "@argos-ci/cypress/task":
      return "export const registerArgosTask = () => undefined;";
    case "@cypress/code-coverage/task.js":
      return "export default () => undefined;";
    case "cypress":
      return "export const defineConfig = (config) => config;";
    case "cypress-image-snapshot/plugin.js":
      return "export const addMatchImageSnapshotPlugin = () => undefined;";
    case "dotenv/config":
      return "export {};";
    default:
      return null;
  }
};

const importBundledCode = async (code) => {
  const moduleUrl = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
  return import(moduleUrl);
};

const loadPinnedCypressConfig = async (esbuild, configPath) => {
  const result = await esbuild.build({
    absWorkingDir: path.dirname(configPath),
    bundle: true,
    entryPoints: [configPath],
    format: "esm",
    logLevel: "silent",
    platform: "node",
    sourcemap: "inline",
    write: false,
    plugins: [
      {
        name: "merman-cypress-config-boundary",
        setup(build) {
          build.onResolve({ filter: /.*/ }, (args) => {
            if (args.importer === "") {
              return null;
            }
            if (args.path === "node:fs" || args.path === "node:path") {
              return { path: args.path, external: true };
            }
            if (configStubSource(args.path) !== null) {
              return { path: args.path, namespace: "merman-config-stub" };
            }
            return {
              errors: [
                {
                  text: `unsupported Cypress config import ${JSON.stringify(args.path)}`,
                  location: args.pluginData?.location,
                },
              ],
            };
          });
          build.onLoad({ filter: /.*/, namespace: "merman-config-stub" }, (args) => ({
            contents: configStubSource(args.path),
            loader: "js",
          }));
        },
      },
    ],
  });
  if (result.outputFiles.length !== 1) {
    fail(`Cypress config build produced ${result.outputFiles.length} outputs`);
  }
  const module = await importBundledCode(result.outputFiles[0].text);
  const specPattern = module.default?.e2e?.specPattern;
  if (specPattern !== "cypress/integration/**/*.{js,ts}") {
    fail(
      `unsupported pinned Cypress specPattern ${JSON.stringify(specPattern)}; collector expects cypress/integration/**/*.{js,ts}`
    );
  }
  return specPattern;
};

const discoverScopeSpecs = async (mermaidRoot, scope, specPattern) => {
  if (specPattern !== "cypress/integration/**/*.{js,ts}") {
    fail(`cannot enumerate unsupported Cypress specPattern ${JSON.stringify(specPattern)}`);
  }
  const integrationRoot = path.join(mermaidRoot, "cypress/integration");
  const allSpecs = (await enumerateFiles(integrationRoot))
    .filter((file) => file.endsWith(".js") || file.endsWith(".ts"))
    .map((file) => path.relative(mermaidRoot, file).split(path.sep).join("/"));
  const selected = [];
  const selectedSet = new Set();
  const addSelected = (spec) => {
    if (!selectedSet.has(spec)) {
      selectedSet.add(spec);
      selected.push(spec);
    }
  };
  for (const selector of scope.selectors) {
    const absolute = path.join(mermaidRoot, selector.path);
    const metadata = await stat(absolute).catch(() => null);
    if (metadata === null) {
      fail(`collector selector is missing: ${selector.path}`);
    }
    if (selector.kind === "file") {
      if (!metadata.isFile() || !allSpecs.includes(selector.path)) {
        fail(`collector file selector is not included by ${specPattern}: ${selector.path}`);
      }
      addSelected(selector.path);
      continue;
    }
    if (!metadata.isDirectory()) {
      fail(`collector directory selector is not a directory: ${selector.path}`);
    }
    const prefix = `${selector.path}/`;
    const matches = allSpecs.filter((spec) => spec.startsWith(prefix));
    if (matches.length === 0) {
      fail(`collector directory selector matched no specs: ${selector.path}`);
    }
    for (const spec of matches) {
      addSelected(spec);
    }
  }
  return selected;
};

const renderHelperModule = (scope) => {
  const exports = [];
  for (const helper of scope.capturedHelpers) {
    if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(helper)) {
      fail(`invalid captured helper name ${JSON.stringify(helper)}`);
    }
    exports.push(
      `export const ${helper} = (...args) => globalThis.__mermanCypressCollector.capture(${JSON.stringify(helper)}, args);`
    );
  }
  for (const helper of scope.passiveHelperImports) {
    if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(helper)) {
      fail(`invalid passive helper name ${JSON.stringify(helper)}`);
    }
    exports.push(
      `export const ${helper} = (...args) => globalThis.__mermanCypressCollector.passive(${JSON.stringify(helper)}, args);`
    );
  }
  return exports.join("\n");
};

const compileSpec = async (esbuild, mermaidRoot, helperPath, sourceSpec, scope) => {
  const entry = path.join(mermaidRoot, sourceSpec);
  const helperWithoutExtension = helperPath.replace(/\.ts$/, "");
  const result = await esbuild.build({
    absWorkingDir: mermaidRoot,
    bundle: true,
    entryPoints: [entry],
    format: "esm",
    logLevel: "silent",
    platform: "node",
    sourcemap: "inline",
    write: false,
    plugins: [
      {
        name: "merman-render-helper-boundary",
        setup(build) {
          build.onResolve({ filter: /.*/ }, (args) => {
            if (args.importer === "") {
              return null;
            }
            const importer = path.relative(mermaidRoot, args.importer).split(path.sep).join("/");
            if (importer !== sourceSpec) {
              return {
                errors: [
                  {
                    text: `collector bundle unexpectedly resolved ${JSON.stringify(args.path)} from ${importer}`,
                  },
                ],
              };
            }
            if (!args.path.startsWith(".")) {
              return {
                errors: [
                  {
                    text: `unsupported import ${JSON.stringify(args.path)} in ${sourceSpec}`,
                  },
                ],
              };
            }
            const resolved = path
              .relative(mermaidRoot, path.resolve(path.dirname(args.importer), args.path))
              .split(path.sep)
              .join("/");
            if (resolved === helperPath || resolved === helperWithoutExtension) {
              return { path: helperPath, namespace: "merman-render-helper" };
            }
            return {
              errors: [
                {
                  text: `unsupported import ${JSON.stringify(args.path)} in ${sourceSpec}; resolved to ${resolved}`,
                },
              ],
            };
          });
          build.onLoad({ filter: /.*/, namespace: "merman-render-helper" }, () => ({
            contents: renderHelperModule(scope),
            loader: "js",
          }));
        },
      },
    ],
  });
  if (result.outputFiles.length !== 1) {
    fail(`${sourceSpec} build produced ${result.outputFiles.length} outputs`);
  }
  return result.outputFiles[0].text;
};

const executeSpec = (code, sourceSpec, scope) =>
  new Promise((resolve, reject) => {
    const worker = new Worker(workerPath, {
      workerData: {
        allowedRuntimeEffects: scope.allowedRuntimeEffects,
        code,
        sourceSpec,
      },
    });
    let settled = false;
    const timeout = setTimeout(async () => {
      if (settled) {
        return;
      }
      settled = true;
      await worker.terminate();
      reject(new Error(`[${sourceSpec}] collector execution exceeded ${scope.timeoutMs}ms`));
    }, scope.timeoutMs);
    worker.once("message", (message) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      if (message.error) {
        reject(new Error(message.error));
      } else {
        resolve(message);
      }
    });
    worker.once("error", (error) => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimeout(timeout);
      reject(error);
    });
    worker.once("exit", (code) => {
      if (!settled && code !== 0) {
        settled = true;
        clearTimeout(timeout);
        reject(new Error(`[${sourceSpec}] collector worker exited ${code}`));
      }
    });
  });

const validateObservedScope = (scopeId, scope, result) => {
  if (result.calls.length !== scope.expectedActiveCalls) {
    fail(
      `${scopeId} collected ${result.calls.length} active calls; expected ${scope.expectedActiveCalls}. Review additions or declare removed identities before changing the scope contract.`
    );
  }
  const skipped = result.registrations.filter((registration) => registration.skipped);
  if (skipped.length !== scope.expectedSkippedRegistrations) {
    fail(
      `${scopeId} collected ${skipped.length} skipped registrations; expected ${scope.expectedSkippedRegistrations}`
    );
  }
  const expectedSkipped = scope.reviewedSkippedRegistrations
    .map((entry) => entry.registration)
    .sort();
  const actualSkipped = skipped.map((entry) => entry.id).sort();
  if (JSON.stringify(expectedSkipped) !== JSON.stringify(actualSkipped)) {
    fail(
      `${scopeId} skipped registrations do not match reviewedSkippedRegistrations: expected ${JSON.stringify(expectedSkipped)}, found ${JSON.stringify(actualSkipped)}`
    );
  }
};

export const collectScope = async (args) => {
  const canonicalWorkspaceRoot = await realpath(args.workspaceRoot);
  const canonicalMermaidRoot = await assertInside(
    canonicalWorkspaceRoot,
    args.mermaidRoot,
    "Mermaid checkout"
  );
  const canonicalScopesPath = await assertInside(
    canonicalWorkspaceRoot,
    args.scopesPath,
    "collector scope catalog"
  );
  const { bytes: scopesBytes, value: config } = await readJson(
    canonicalScopesPath,
    "collector scope catalog"
  );
  const scope = validateScopeConfig(config, args.scope);

  const actualCommit = runText("git", ["rev-parse", "HEAD"], canonicalMermaidRoot);
  if (actualCommit !== config.mermaidSourceCommit) {
    fail(
      `Mermaid checkout commit drift: expected ${config.mermaidSourceCommit}, found ${actualCommit}`
    );
  }
  const dirty = runText("git", ["status", "--short"], canonicalMermaidRoot);
  if (dirty !== "") {
    fail(`Mermaid checkout must be clean before collection:\n${dirty}`);
  }

  const packageJsonPath = path.join(canonicalMermaidRoot, "package.json");
  const { value: packageJson } = await readJson(packageJsonPath, "Mermaid package manifest");
  const packageManager = `pnpm@${config.pnpmVersion}`;
  if (
    typeof packageJson.packageManager !== "string" ||
    !packageJson.packageManager.startsWith(`${packageManager}+`)
  ) {
    fail(
      `Mermaid packageManager must pin ${packageManager} with integrity, found ${JSON.stringify(packageJson.packageManager)}`
    );
  }
  const pinnedNode = (await readFile(path.join(canonicalMermaidRoot, ".node-version"), "utf8")).trim();
  if (pinnedNode !== config.nodeVersion) {
    fail(`Mermaid .node-version must be ${config.nodeVersion}, found ${pinnedNode}`);
  }
  if (process.version !== `v${config.nodeVersion}`) {
    fail(`collector requires Node v${config.nodeVersion}, found ${process.version}`);
  }
  const pnpmVersion = runText("pnpm", ["--version"], canonicalMermaidRoot);
  if (pnpmVersion !== config.pnpmVersion) {
    fail(`collector requires pnpm ${config.pnpmVersion}, found ${pnpmVersion}`);
  }

  const requireFromMermaid = createRequire(packageJsonPath);
  let esbuild;
  let esbuildVersion;
  try {
    esbuild = requireFromMermaid("esbuild");
    esbuildVersion = requireFromMermaid("esbuild/package.json").version;
  } catch (error) {
    fail(
      `pinned Mermaid dependencies are not installed; run pnpm ${config.pnpmVersion} install --frozen-lockfile --ignore-scripts in ${canonicalMermaidRoot}: ${error.message}`
    );
  }
  if (esbuildVersion !== config.esbuildVersion) {
    fail(`collector requires esbuild ${config.esbuildVersion}, found ${esbuildVersion}`);
  }

  const testConfigPath = await assertInside(
    canonicalMermaidRoot,
    path.join(canonicalMermaidRoot, config.testConfig),
    "Cypress config"
  );
  const helperPath = await assertInside(
    canonicalMermaidRoot,
    path.join(canonicalMermaidRoot, config.renderHelper),
    "Cypress render helper"
  );
  const specPattern = await loadPinnedCypressConfig(esbuild, testConfigPath);
  const sourceSpecs = await discoverScopeSpecs(canonicalMermaidRoot, scope, specPattern);

  const registrations = [];
  const calls = [];
  const runtimeEffects = [];
  const sourceFiles = [];
  for (const sourceSpec of sourceSpecs) {
    const sourcePath = await assertInside(
      canonicalMermaidRoot,
      path.join(canonicalMermaidRoot, sourceSpec),
      "Cypress source spec"
    );
    const sourceBytes = await readFile(sourcePath);
    sourceFiles.push({ path: sourceSpec, sha256: sha256(sourceBytes) });
    let code;
    try {
      code = await compileSpec(esbuild, canonicalMermaidRoot, config.renderHelper, sourceSpec, scope);
    } catch (error) {
      fail(`${sourceSpec} failed to compile through pinned esbuild ${esbuildVersion}: ${error.message}`);
    }
    const observed = await executeSpec(code, sourceSpec, scope);
    for (const registration of observed.registrations) {
      registrations.push({ sourceSpec, ...registration });
    }
    for (const [sourceIndex, call] of observed.calls.entries()) {
      calls.push({
        sourceSpec,
        sourceOrdinal: sourceIndex + 1,
        ordinal: calls.length + 1,
        ...call,
      });
    }
    for (const effect of observed.runtimeEffects) {
      runtimeEffects.push({ sourceSpec, ...effect });
    }
  }

  const supplementalFixtures = [];
  for (const fixture of scope.supplementalFixtures) {
    const fixturePath = await assertInside(
      canonicalWorkspaceRoot,
      path.join(canonicalWorkspaceRoot, fixture),
      "supplemental fixture"
    );
    supplementalFixtures.push({ path: fixture, sha256: sha256(await readFile(fixturePath)) });
  }

  const result = { registrations, calls, runtimeEffects };
  validateObservedScope(args.scope, scope, result);

  const collectorFiles = [];
  for (const file of [scriptPath, workerPath, canonicalScopesPath]) {
    const relative = path.relative(canonicalWorkspaceRoot, file).split(path.sep).join("/");
    collectorFiles.push({ path: relative, sha256: sha256(await readFile(file)) });
  }
  collectorFiles.sort((left, right) => left.path.localeCompare(right.path));

  const [configBytes, helperBytes, lockBytes] = await Promise.all([
    readFile(testConfigPath),
    readFile(helperPath),
    readFile(path.join(canonicalMermaidRoot, "pnpm-lock.yaml")),
  ]);
  return {
    schemaVersion: 1,
    kind: "merman-upstream-cypress-collection",
    scope: {
      id: args.scope,
      description: scope.description,
      expectedActiveCalls: scope.expectedActiveCalls,
      expectedSkippedRegistrations: scope.expectedSkippedRegistrations,
      reviewedSkippedRegistrations: scope.reviewedSkippedRegistrations,
      reviewedRemovals: scope.reviewedRemovals,
    },
    source: {
      package: "mermaid",
      version: config.mermaidVersion,
      tag: config.mermaidSourceTag,
      commit: config.mermaidSourceCommit,
      testConfig: {
        path: config.testConfig,
        sha256: sha256(configBytes),
        specPattern,
      },
      renderHelper: {
        path: config.renderHelper,
        sha256: sha256(helperBytes),
      },
      specs: sourceFiles,
      supplementalFixtures,
    },
    collector: {
      files: collectorFiles,
      scopeCatalogSha256: sha256(scopesBytes),
      nodeVersion: process.version.slice(1),
      pnpmVersion,
      esbuildVersion,
      upstreamLock: {
        path: "pnpm-lock.yaml",
        sha256: sha256(lockBytes),
      },
    },
    registrations,
    calls,
    runtimeEffects,
  };
};

const main = async () => {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(
      "usage: node tools/upstreams/cypress-collector/collect.mjs --scope <new-family|flowchart-elk> --output <path> [--workspace-root <path>] [--mermaid-root <path>]\n"
    );
    return;
  }
  const result = await collectScope(args);
  await mkdir(path.dirname(args.output), { recursive: true });
  await writeFile(args.output, `${JSON.stringify(result, null, 2)}\n`, { flag: "w" });
  process.stdout.write(
    `collected scope=${args.scope} specs=${result.source.specs.length} registrations=${result.registrations.length} calls=${result.calls.length} output=${args.output}\n`
  );
};

if (path.resolve(process.argv[1] ?? "") === scriptPath) {
  main().catch((error) => {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
