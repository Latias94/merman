import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { validateTransportIdentityJson } from "../src/errors.mjs";
import { BINDING_OPTIONS_SCHEMA_VERSION } from "../src/generated/binding-contract.mjs";
import { nodeLoaderPackageVersion, resolveNodeTarget } from "../src/native-loader.mjs";
import { validateRuntimeCatalog } from "../src/engine.mjs";
import { NODE_WIRE_CONTRACT, localLayoutCapability, nodeWireContract } from "../src/transport-contract.mjs";
import { readLocalLayoutFlag } from "./local-layout.mjs";
import { replaceDirectory } from "./replace-directory.mjs";
import { svgTransportEvidence } from "./benchmark/svg-signature.mjs";
import { digestJson, stableJson } from "./stable-json.mjs";
import { assertMathSvg } from "./theme-authoring-smoke.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = path.resolve(nodeRoot, "..", "..");
const descriptor = readJson(path.join(nodeRoot, "candidate-builds.json"));
const packageSurfacePath = path.join(nodeRoot, "package-surfaces.json");
const packageSurface = readJson(packageSurfacePath);
assertDescriptor();
const capabilityDescriptorPath = path.join(
  repositoryRoot,
  descriptor.capability_recipe.descriptor,
);
const capabilitySurface = readJson(capabilityDescriptorPath);
const artifactsRoot = path.join(nodeRoot, "artifacts");
const cargoPackage = "merman-node-candidate";
export const cargoLockPath = path.join(repositoryRoot, "Cargo.lock");
const requireFromBuild = createRequire(import.meta.url);
const WINDOWS_MSVC_REPRODUCIBLE_LINK_CONFIG =
  'target.x86_64-pc-windows-msvc.rustflags=["-C","link-arg=/Brepro"]';

if (isMainModule()) {
  try {
    const selection = parseArgs(process.argv.slice(2));
    buildCandidate(selection);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}

export function buildCandidate({ candidate, target = null, localLayout = null }) {
  const resolvedTarget = candidate === "napi" ? target ?? resolveNodeTarget() : null;
  const recipe = resolveCandidateRecipe(candidate, resolvedTarget, { localLayout });
  validateCandidateBuildHost(recipe, candidate === "napi" ? resolveNodeTarget() : null);
  const dependencyClosure = resolveCargoClosure(recipe);
  const before = collectLocalInputEntries(dependencyClosure);
  const output = candidateArtifactDirectory(recipe);
  const outputParent = path.dirname(output);
  mkdirSync(outputParent, { recursive: true });
  const stage = mkdtempSync(path.join(outputParent, `.stage-${candidate}-`));

  try {
    if (candidate === "napi") buildNapi(stage, recipe);
    else buildNodeWasm(stage, recipe);
    normalizeArtifacts(stage, candidate);
    const runtime = probeCandidateRuntime(stage, recipe);

    const afterClosure = resolveCargoClosure(recipe);
    const after = collectLocalInputEntries(afterClosure);
    if (stableJson(before) !== stableJson(after)) {
      throw new Error("Node candidate source inputs changed during the build; rerun it.");
    }
    if (dependencyClosure.digest !== afterClosure.digest) {
      throw new Error("Node candidate dependency closure changed during the build; rerun it.");
    }
    writeBuildEnvironmentEvidence(stage, recipe);
    writeBuildReceipt(stage, recipe, before, dependencyClosure, runtime);
    replaceDirectory(stage, output);
    console.log(`[merman-node] built ${candidate} (${recipe.cargoProfile}) at ${output}`);
  } catch (error) {
    rmSync(stage, { recursive: true, force: true });
    throw error;
  }
}

export function candidateArtifactDirectory(recipe) {
  if (recipe.localLayout !== null) {
    return path.join(localArtifactRoot(recipe.localLayout), "artifacts", recipe.targetId ?? recipe.candidate);
  }
  if (recipe.candidate === "napi") return path.join(artifactsRoot, "napi", recipe.targetId);
  return path.join(artifactsRoot, "node-wasm");
}

export function localArtifactRoot(localLayout) {
  if (localLayoutCapability(localLayout) === null) throw new Error("A local layout is required.");
  return path.join(repositoryRoot, "target", "bench", `node-${localLayout}`);
}

function buildNapi(stage, recipe) {
  const cli = path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js");
  assertFile(cli, "pinned @napi-rs/cli; run npm ci first");
  const invocation = candidateBuildInvocation(recipe, stage);
  run(invocation.command, invocation.args, invocation.env);
}

function buildNodeWasm(stage, recipe) {
  const invocation = candidateBuildInvocation(recipe, stage);
  run(invocation.command, invocation.args);
}

export function candidateBuildInvocation(recipe, stage, environment = process.env) {
  const featureArgument = recipe.cargoFeatures.join(",");
  const cargoArguments = ["--locked", "-j1"];
  if (recipe.rustTarget === "x86_64-pc-windows-msvc") {
    cargoArguments.push("--config", WINDOWS_MSVC_REPRODUCIBLE_LINK_CONFIG);
  }
  if (recipe.candidate === "napi") {
    return {
      command: process.execPath,
      // The CLI defaults to a cross-linker name even on a matching native Alpine host.
      env: recipe.rustTarget === "aarch64-unknown-linux-musl"
        ? {
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER:
            environment.CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER ?? "cc",
        }
        : {},
      args: [
        path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
        "build",
        "--cwd",
        repositoryRoot,
        "--manifest-path",
        descriptor.cargo.manifest,
        "--package",
        cargoPackage,
        "--package-json-path",
        path.join(nodeRoot, "package.json"),
        "--target",
        recipe.rustTarget,
        "--output-dir",
        stage,
        "--platform",
        "--no-js",
        "--dts",
        "native-internal.d.ts",
        "--profile",
        recipe.cargoProfile,
        "--no-default-features",
        "--features",
        featureArgument,
        "--",
        ...cargoArguments,
      ],
    };
  }
  if (recipe.candidate === "node-wasm") {
    return {
      command: "wasm-pack",
      args: [
        "build",
        path.dirname(path.join(repositoryRoot, descriptor.cargo.manifest)),
        "--target",
        recipe.wasmPackTarget,
        "--release",
        "--no-pack",
        "--out-dir",
        stage,
        "--",
        ...cargoArguments,
        "--no-default-features",
        "--features",
        featureArgument,
      ],
    };
  }
  throw new Error(`Unknown candidate: ${recipe.candidate}.`);
}

export function validateCandidateBuildHost(recipe, currentTarget) {
  if (recipe.candidate !== "napi") return;
  if (recipe.targetId !== currentTarget) {
    throw new Error(
      `The ${recipe.targetId} Node package must be built and probed on its matching runtime host; current host is ${currentTarget}.`,
    );
  }
}

function normalizeArtifacts(stage, candidate) {
  if (candidate === "napi") {
    const nodeFiles = walkFiles(stage).filter((file) => file.endsWith(".node"));
    if (nodeFiles.length !== 1) {
      throw new Error(`napi build produced ${nodeFiles.length} .node files; expected exactly one.`);
    }
    const canonical = path.join(stage, "merman.node");
    if (path.resolve(nodeFiles[0]) !== path.resolve(canonical)) renameSync(nodeFiles[0], canonical);
    return;
  }
  assertFile(path.join(stage, "merman_node.js"), "Node-targeted wasm-bindgen loader");
  assertFile(path.join(stage, "merman_node_bg.wasm"), "Node-targeted WASM binary");
  const wasmFiles = walkFiles(stage).filter((file) => file.endsWith(".wasm"));
  if (wasmFiles.length !== 1) {
    throw new Error(`Node WASM build produced ${wasmFiles.length} WASM files; expected exactly one.`);
  }
  writeFileSync(
    path.join(stage, "package.json"),
    `${JSON.stringify({ private: true, type: "commonjs" }, null, 2)}\n`,
  );
}

function writeBuildReceipt(
  stage,
  recipe,
  inputEntries,
  dependencyClosure,
  runtime,
) {
  const sourceDigest = digestJson(inputEntries);
  const bindingContractEntries = collectBindingContractEntries(inputEntries);
  const bindingContractDigest = digestJson(bindingContractEntries);
  const cargoLockDigest = inputEntries.find((entry) => entry.path === "Cargo.lock").sha256;
  const tools = {
    cargo: runCapture("cargo", ["--version"]),
    node: process.version,
    rustc: runCapture("rustc", ["--version", "--verbose"]),
    transport_builder:
      recipe.candidate === "napi"
        ? runCapture(process.execPath, [
            path.join(nodeRoot, "node_modules", "@napi-rs", "cli", "dist", "cli.js"),
            "--version",
          ])
        : runCapture("wasm-pack", ["--version"]),
  };
  const config = {
    candidate: recipe.candidate,
    target: recipe.targetId,
    rust_target: recipe.rustTarget,
    wasm_pack_target: recipe.wasmPackTarget,
    glibc_floor: recipe.glibcFloor,
    cargo_profile: recipe.cargoProfile,
    default_features: false,
    capability_recipe: {
      descriptor: descriptor.capability_recipe.descriptor,
      target: descriptor.capability_recipe.target,
      capabilities: recipe.capabilityFeatures,
    },
    features: recipe.cargoFeatures,
    diagram_families: recipe.diagramFamilies,
  };
  const receipt = {
    schema_version: 2,
    config,
    commit: runCapture("git", ["rev-parse", "HEAD"]),
    source_digest: sourceDigest,
    cargo_lock_digest: cargoLockDigest,
    binding_contract_digest: bindingContractDigest,
    dependency_closure: dependencyClosure,
    input_digest: digestJson({
      cargo_lock_digest: cargoLockDigest,
      config,
      dependency_closure_digest: dependencyClosure.digest,
      source_digest: sourceDigest,
      tools,
    }),
    runtime,
    tools,
    artifacts: artifactEntries(stage),
  };
  writeFileSync(path.join(stage, "build-receipt.json"), `${JSON.stringify(receipt, null, 2)}\n`);
}

function writeBuildEnvironmentEvidence(stage, recipe) {
  if (recipe.glibcFloor === null) return;
  const runtimeVersion = process.report?.getReport?.()?.header?.glibcVersionRuntime;
  if (typeof runtimeVersion !== "string" || runtimeVersion.length === 0) {
    throw new Error("The Linux GNU candidate build must record its runtime glibc version.");
  }
  if (compareGlibcVersions(runtimeVersion, recipe.glibcFloor) > 0) {
    throw new Error(
      `The Linux GNU candidate build ran on glibc ${runtimeVersion}, above the ${recipe.glibcFloor} compatibility floor.`,
    );
  }
  writeFileSync(
    path.join(stage, "build-environment.json"),
    `${JSON.stringify({
      schema_version: 1,
      target: recipe.targetId,
      libc: "glibc",
      version: runtimeVersion,
      floor: recipe.glibcFloor,
    }, null, 2)}\n`,
  );
}

function compareGlibcVersions(left, right) {
  const parse = (value) => {
    const match = /^(\d+)\.(\d+)$/.exec(value);
    if (!match) throw new Error(`Invalid glibc version: ${value}.`);
    return [Number(match[1]), Number(match[2])];
  };
  const [leftMajor, leftMinor] = parse(left);
  const [rightMajor, rightMinor] = parse(right);
  return leftMajor - rightMajor || leftMinor - rightMinor;
}

export function probeCandidateRuntime(stage, recipe) {
  const artifact = path.join(
    stage,
    recipe.candidate === "napi" ? "merman.node" : "merman_node.js",
  );
  const binding = requireFromBuild(artifact);
  if (typeof binding?.transportIdentityJson !== "function") {
    throw new Error(`${recipe.candidate} candidate does not export transportIdentityJson().`);
  }
  validateTransportIdentityJson(binding.transportIdentityJson(), {
    expectedPackageVersion: nodeLoaderPackageVersion(),
    expectedTransport: recipe.candidate === "napi" ? "napi" : "wasm",
    localLayout: recipe.localLayout,
  });
  const Engine = recipe.candidate === "napi"
    ? binding?.NativeEngine ?? binding?.default?.NativeEngine
    : binding?.WasmEngine ?? binding?.default?.WasmEngine;
  if (
    typeof Engine !== "function" ||
    typeof Engine.prototype?.execute !== "function" ||
    typeof Engine.prototype?.executeSync !== "function" ||
    typeof Engine.prototype?.runtimeCatalogJson !== "function" ||
    typeof Engine.prototype?.metadataJson !== "function" ||
    typeof Engine.prototype?.dispose !== "function"
  ) {
    throw new Error(`${recipe.candidate} candidate does not export its complete engine contract.`);
  }
  const engine = new Engine(JSON.stringify({
    version: BINDING_OPTIONS_SCHEMA_VERSION,
    runtime_policy: "deterministic",
    resources: { profile: "interactive" },
  }));
  let disposed = false;
  try {
    const catalog = validateRuntimeCatalog(engine.runtimeCatalogJson(), recipe);
    const capabilities = catalog?.capabilities;
    const capabilityIds = sortedUniqueStrings(
      capabilities?.capability_ids,
      "runtime capability IDs",
    );
    const outputIds = sortedUniqueStrings(
      capabilities?.output_ids,
      "runtime output IDs",
    );
    const operationIds = sortedUniqueStrings(
      capabilities?.operation_ids,
      "runtime operation IDs",
    );
    const systemAdapterIds = sortedUniqueStrings(
      capabilities?.system_adapter_ids,
      "runtime system adapter IDs",
      { allowEmpty: true },
    );
    const expectedRuntime = resolveCandidateRuntimeContract(recipe);
    if (
      catalog.schema_version !== 1 ||
      catalog.transport_api_version !== NODE_WIRE_CONTRACT.transport_api_version ||
      catalog.package_version !== packageSurface.version ||
      stableJson(capabilityIds) !== stableJson(expectedRuntime.capabilityIds) ||
      stableJson(outputIds) !== stableJson(expectedRuntime.outputIds) ||
      stableJson(operationIds) !== stableJson(expectedRuntime.operationIds) ||
      stableJson(systemAdapterIds) !== stableJson(expectedRuntime.systemAdapterIds) ||
      stableJson(capabilities?.text_measurement?.provider_ids) !==
        stableJson(expectedRuntime.textMeasurementProviderIds) ||
      !Number.isSafeInteger(catalog.registry?.diagram_family_count) ||
      catalog.registry.diagram_family_count < 1 ||
      stableJson(catalog.registry.diagram_families) !== stableJson(expectedRuntime.diagramFamilies)
    ) {
      throw new Error(`${recipe.candidate} runtime catalog disagrees with its capability recipe.`);
    }

    const source = "flowchart TD\nA --> B";
    const semanticRequest = JSON.stringify({
      operation_id: "semantic-json",
      source,
      uri: null,
      options_json: JSON.stringify({ version: BINDING_OPTIONS_SCHEMA_VERSION }),
    });
    const semantic = parseWireResponse(engine.executeSync(semanticRequest));
    if (
      semantic.ok !== true ||
      semantic.result?.operation_id !== "semantic-json" ||
      semantic.result?.media_type !== "application/json" ||
      typeof semantic.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime semantic JSON probe failed.`);
    }
    JSON.parse(semantic.result.data);
    const asyncProbe = probeCandidateAsyncLifecycle(artifact, recipe, semanticRequest);
    const svgPlan = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg-plan-json",
      source,
      uri: null,
      options_json: JSON.stringify({ version: BINDING_OPTIONS_SCHEMA_VERSION }),
    })));
    if (
      svgPlan.ok !== true ||
      svgPlan.result?.operation_id !== "svg-plan-json" ||
      svgPlan.result?.media_type !== "application/json" ||
      typeof svgPlan.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG capability-plan probe failed.`);
    }
    const parsedSvgPlan = JSON.parse(svgPlan.result.data);
    if (
      parsedSvgPlan?.schema_version !== 1 ||
      parsedSvgPlan?.planned_operation_id !== "svg" ||
      !Array.isArray(parsedSvgPlan?.required_capability_ids) ||
      !Array.isArray(parsedSvgPlan?.missing_capability_ids) ||
      typeof parsedSvgPlan?.ready !== "boolean"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG capability-plan payload is invalid.`);
    }
    const svgRequest = {
      operation_id: "svg",
      source,
      uri: null,
      options_json: JSON.stringify({ resources: { limits: { max_source_bytes: 4096 } } }),
    };
    const success = parseWireResponse(engine.executeSync(JSON.stringify(svgRequest)));
    if (
      success.ok !== true ||
      success.result?.operation_id !== "svg" ||
      success.result?.media_type !== "image/svg+xml" ||
      typeof success.result?.data !== "string"
    ) {
      throw new Error(`${recipe.candidate} runtime SVG probe failed.`);
    }
    if (recipe.candidate === "napi") {
      if (
        typeof engine.executeSvg !== "function" ||
        typeof engine.executeSvgSync !== "function" ||
        engine.executeSvgSync(svgRequest.source, svgRequest.options_json) !== success.result.data
      ) {
        throw new Error("Native candidate direct SVG must match the wire response.");
      }
      const deadline = engine.executeSvgSync(svgRequest.source, svgRequest.options_json, 0);
      if (parseWireResponse(deadline?.errorJson).error?.details?.cancellation?.reason !== "deadline_exceeded") {
        throw new Error("Native candidate direct SVG must preserve deadline errors.");
      }
    }
    const shoalRequest = {
      ...svgRequest,
      source: "---\nconfig:\n  layout: shoal\n---\nflowchart LR\nA --> B\nB --> A",
    };
    const shoal = parseWireResponse(engine.executeSync(JSON.stringify(shoalRequest)));
    if (recipe.localLayout === "shoal") {
      if (
        shoal.ok !== true || shoal.result?.media_type !== "image/svg+xml" ||
        typeof shoal.result.data !== "string" ||
        !shoal.result.data.includes("L_A_B_0") || !shoal.result.data.includes("L_B_A_0") ||
        (recipe.candidate === "napi" &&
          engine.executeSvgSync(shoalRequest.source, shoalRequest.options_json) !== shoal.result.data)
      ) {
        throw new Error("The local Node candidate could not render the explicit Shoal layout.");
      }
    } else if (
      shoal.ok !== false || shoal.error?.kind !== "missing-capability" ||
      shoal.error?.capability_id !== "layout-shoal"
    ) {
      throw new Error("The Node candidate must report unselected Shoal as a missing capability.");
    }
    if (recipe.localLayout === "grid" || recipe.localLayout === "layered") {
      probeLocalMermaidLayout(engine, recipe, svgRequest);
    }
    const svgEvidence = svgTransportEvidence(success.result.data);
    const math = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg",
      source: 'flowchart LR\nA["$$x^2$$"]',
      options_json: JSON.stringify({ site_config: { htmlLabels: true } }),
    })));
    if (math.ok !== true || math.result?.media_type !== "image/svg+xml") {
      throw new Error(`${recipe.candidate} runtime formula probe failed.`);
    }
    const formulaViewports = assertMathSvg(math.result.data);
    const limited = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "svg",
      source,
      uri: null,
      options_json: JSON.stringify({ resources: { limits: { max_source_bytes: 4 } } }),
    })));
    if (
      limited.ok !== false ||
      limited.error?.code_name !== "MERMAN_RESOURCE_LIMIT_EXCEEDED"
    ) {
      throw new Error(`${recipe.candidate} runtime ignored request-local options JSON.`);
    }
    const unknown = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "bitmap",
      source,
      uri: null,
    })));
    if (unknown.ok !== false || unknown.error?.kind !== "unknown-operation") {
      throw new Error(`${recipe.candidate} runtime lost its typed unknown-operation error.`);
    }
    const missing = parseWireResponse(engine.executeSync(JSON.stringify({
      operation_id: "png",
      source,
      uri: null,
    })));
    if (
      missing.ok !== false ||
      missing.error?.kind !== "missing-capability" ||
      missing.error?.capability_id !== "png"
    ) {
      throw new Error(`${recipe.candidate} runtime lost its typed missing-capability error.`);
    }
    const runtime = {
      catalog_digest: digestJson(catalog),
      catalog,
      probe: {
        missing_capability_id: missing.error.capability_id,
        async_semantic_json_bytes: asyncProbe.semantic_json_bytes,
        semantic_json_bytes: Buffer.byteLength(semantic.result.data),
        svg_plan_json_bytes: Buffer.byteLength(svgPlan.result.data),
        svg_bytes: Buffer.byteLength(success.result.data),
        math_svg_bytes: Buffer.byteLength(math.result.data),
        math_formula_viewports: formulaViewports,
        svg_structure_sha256: svgEvidence.structure_sha256,
        svg_geometry_sha256: svgEvidence.geometry_sha256,
        unknown_operation_kind: unknown.error.kind,
        request_options_limit_code_name: limited.error.code_name,
      },
    };
    engine.dispose();
    disposed = true;
    assertDisposedCandidateEngine(engine, recipe.candidate, semanticRequest);
    engine.dispose();
    return runtime;
  } finally {
    if (!disposed) {
      try {
        engine.dispose?.();
      } catch {
        // Preserve the probe failure that made the staged candidate unusable.
      }
    }
  }
}

function probeLocalMermaidLayout(engine, recipe, svgRequest) {
  const { localLayout } = recipe;
  const capability = localLayoutCapability(localLayout);
  const source = `---\nconfig:\n  layout: ${localLayout}\n---\nflowchart LR\nA[Start] -->|Approved| B[Done]\n`;
  const request = { ...svgRequest, source };
  const execute = (operationId, mediaType) => {
    const response = parseWireResponse(engine.executeSync(JSON.stringify({
      ...request,
      operation_id: operationId,
    })));
    if (
      response.ok !== true || response.result?.operation_id !== operationId ||
      response.result?.media_type !== mediaType || typeof response.result.data !== "string"
    ) {
      throw new Error(`The local Node ${localLayout} candidate failed ${operationId}.`);
    }
    return response.result.data;
  };
  const plan = JSON.parse(execute("svg-plan-json", "application/json"));
  const layout = JSON.parse(execute("layout-json", "application/json"));
  if (
    plan?.ready !== true || stableJson(plan.required_capability_ids) !== stableJson([capability]) ||
    stableJson(plan.missing_capability_ids) !== "[]" ||
    layout?.meta?.effective_config?.layout !== localLayout || !layout.layout?.FlowchartV2
  ) {
    throw new Error(`The local Node ${localLayout} candidate did not select the requested layout.`);
  }
  const svg = execute("svg", "image/svg+xml");
  svgTransportEvidence(svg);
  if (recipe.candidate === "napi" && engine.executeSvgSync(source, request.options_json) !== svg) {
    throw new Error(`The native ${localLayout} direct SVG differs from the wire response.`);
  }
}

function assertDisposedCandidateEngine(engine, candidate, requestJson) {
  for (const [method, invoke] of [
    ["executeSync", () => engine.executeSync(requestJson)],
    ["runtimeCatalogJson", () => engine.runtimeCatalogJson()],
    ["metadataJson", () => engine.metadataJson("supported-diagrams")],
  ]) {
    let cause;
    try {
      invoke();
    } catch (error) {
      cause = error;
    }
    const envelope = cause === undefined
      ? null
      : parseWireResponse(cause instanceof Error ? cause.message : String(cause));
    if (
      envelope?.ok !== false ||
      envelope.error?.code_name !== "MERMAN_INVALID_ARGUMENT" ||
      envelope.error?.kind !== "generic" ||
      !/disposed/i.test(envelope.error?.message ?? "")
    ) {
      throw new Error(`${candidate} candidate ${method} did not fail closed after dispose.`);
    }
  }
}

function probeCandidateAsyncLifecycle(artifact, recipe, requestJson) {
  const script = path.join(nodeRoot, "scripts", "probe-candidate-async.mjs");
  const result = spawnSync(
    process.execPath,
    [
      script,
      "--artifact",
      artifact,
      "--candidate",
      recipe.candidate,
      "--request-json",
      requestJson,
      ...(recipe.localLayout === null ? [] : [`--local-${recipe.localLayout}`]),
    ],
    {
      cwd: nodeRoot,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
      timeout: 30_000,
    },
  );
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${recipe.candidate} async candidate probe failed: ${result.stderr.trim() || result.stdout.trim()}`,
    );
  }
  try {
    const evidence = JSON.parse(result.stdout);
    if (!Number.isSafeInteger(evidence.semantic_json_bytes) || evidence.semantic_json_bytes < 1) {
      throw new Error();
    }
    return evidence;
  } catch {
    throw new Error(`${recipe.candidate} async candidate probe returned invalid evidence.`);
  }
}

function parseWireResponse(value) {
  try {
    const parsed = JSON.parse(value);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error();
    return parsed;
  } catch {
    throw new Error("Node candidate returned an invalid wire response.");
  }
}

function sortedUniqueStrings(value, label, { allowEmpty = false } = {}) {
  if (
    !Array.isArray(value) ||
    (!allowEmpty && value.length === 0) ||
    value.some((item) => typeof item !== "string" || item.length === 0) ||
    stableJson(value) !== stableJson([...new Set(value)].sort())
  ) {
    throw new Error(`${label} must be sorted, unique strings.`);
  }
  return value;
}

function collectBindingContractEntries(inputEntries) {
  // A candidate receipt keeps its complete artifact closure separately. This digest
  // intentionally excludes transport wrappers and binds the shared Rust behavior.
  const transportPrefixes = ["crates/merman-node/", "platforms/node/"];
  const entries = inputEntries.filter(
    (entry) => !transportPrefixes.some((prefix) => entry.path.startsWith(prefix)),
  );
  if (entries.length === 0) {
    throw new Error("Node candidate receipt has no shared bindings-contract inputs.");
  }
  return entries;
}

function resolveCargoClosure(recipe) {
  const sharedArgs = [
    "--locked",
    "--manifest-path",
    path.join(repositoryRoot, descriptor.cargo.manifest),
    "--no-default-features",
    "--features",
    recipe.cargoFeatures.join(","),
  ];
  const host = /^host:\s+(\S+)$/m.exec(runCapture("rustc", ["--version", "--verbose"]))?.[1];
  if (!host) throw new Error("Rust compiler did not report its host target.");
  // Cargo tree includes host build dependencies even for a different artifact target.
  // Filter metadata to the union so every selected tree entry can resolve its package ID.
  const platforms = [...new Set([recipe.rustTarget, host])];
  const metadata = JSON.parse(runCapture("cargo", [
    "metadata",
    "--format-version",
    "1",
    ...platforms.flatMap((platform) => ["--filter-platform", platform]),
    ...sharedArgs,
  ]));
  validateCandidatePackageVersions(metadata);
  // Metadata describes the whole workspace. The selected tree excludes its unrelated members,
  // inactive optional dependencies and dev edges while retaining build dependencies.
  const tree = runCapture("cargo", [
    "tree", "--package", cargoPackage, "--target", recipe.rustTarget,
    "--edges", "normal,build", "--prefix", "none", "--format", "{p}", "--color", "never",
    ...sharedArgs,
  ]);
  return candidateDependencyClosure(metadata, tree, recipe);
}

export function collectLocalInputEntries(dependencyClosure) {
  const files = new Set([
    path.join(repositoryRoot, "Cargo.toml"),
    cargoLockPath,
    capabilityDescriptorPath,
    path.join(nodeRoot, "candidate-builds.json"),
    packageSurfacePath,
    path.join(nodeRoot, "package.json"),
    path.join(nodeRoot, "package-lock.json"),
    path.join(nodeRoot, "scripts", "benchmark", "svg-signature.mjs"),
    path.join(nodeRoot, "scripts", "build-candidate.mjs"),
    path.join(nodeRoot, "scripts", "local-layout.mjs"),
    path.join(nodeRoot, "scripts", "probe-candidate-async.mjs"),
    path.join(nodeRoot, "scripts", "replace-directory.mjs"),
    path.join(nodeRoot, "scripts", "stable-json.mjs"),
    path.join(nodeRoot, "scripts", "theme-authoring-smoke.mjs"),
  ]);
  for (const file of walkFiles(path.join(nodeRoot, "src"))) files.add(file);
  for (const { source } of dependencyClosure.packages) {
    if (!source.startsWith("path:")) continue;
    const root = path.resolve(repositoryRoot, source.slice("path:".length));
    for (const file of walkFiles(root, { skipBuildOutputs: true })) files.add(file);
  }
  return [...files]
    .filter((file) => existsSync(file) && statSync(file).isFile())
    .map((file) => ({
      path: path.relative(repositoryRoot, file).split(path.sep).join("/"),
      bytes: statSync(file).size,
      sha256: digestFile(file),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

function artifactEntries(stage) {
  return walkFiles(stage)
    .filter((file) => path.basename(file) !== "build-receipt.json")
    .map((file) => ({
      path: path.relative(stage, file).split(path.sep).join("/"),
      bytes: statSync(file).size,
      sha256: digestFile(file),
    }))
    .sort((left, right) => left.path.localeCompare(right.path));
}

export function resolveCandidateRecipe(candidate, target, { localLayout = null } = {}) {
  assertDescriptor();
  const capabilityFeatures = candidateCapabilityFeatures(localLayout);
  if (candidate === "node-wasm") {
    const wasm = descriptor.candidates["node-wasm"];
    return completeCandidateRecipe(
      {
        candidate,
        targetId: null,
        rustTarget: wasm.rust_target,
        cargoProfile: wasm.profile,
        localLayout,
        transportFeature: wasm.transport_feature,
        wasmPackTarget: wasm.wasm_pack_target,
        glibcFloor: null,
      },
      capabilityFeatures,
    );
  }
  if (candidate !== "napi") throw new Error(`Unknown candidate: ${candidate}.`);
  const nativeTarget = descriptor.candidates.napi.targets.find((item) => item.id === target);
  if (!nativeTarget) throw new Error(`Unknown napi target: ${target}.`);
  return completeCandidateRecipe(
    {
      candidate,
      targetId: target,
      rustTarget: nativeTarget.rust_target,
      cargoProfile: descriptor.candidates.napi.profile,
      localLayout,
      transportFeature: descriptor.candidates.napi.transport_feature,
      wasmPackTarget: null,
      glibcFloor: nativeTarget.glibc_floor ?? null,
    },
    capabilityFeatures,
  );
}

function completeCandidateRecipe(recipe, capabilityFeatures) {
  if (
    typeof recipe.transportFeature !== "string" ||
    !recipe.transportFeature.startsWith("transport-")
  ) {
    throw new Error(`Invalid transport feature for ${recipe.candidate}.`);
  }
  if (recipe.glibcFloor !== null && typeof recipe.glibcFloor !== "string") {
    throw new Error(`Invalid glibc floor for ${recipe.candidate}.`);
  }
  if (capabilityFeatures.includes(recipe.transportFeature)) {
    throw new Error(
      `Transport feature collides with the candidate capability recipe: ${recipe.transportFeature}.`,
    );
  }
  return {
    ...recipe,
    capabilityFeatures,
    diagramFamilies: candidateDiagramFamilies(),
    cargoFeatures: [...descriptor.cargo.features, ...capabilityFeatures, recipe.transportFeature].sort(),
  };
}

function candidateCapabilityFeatures(localLayout) {
  const recipe = descriptor.capability_recipe;
  if (
    capabilitySurface.schema_version !== 1 ||
    !Array.isArray(capabilitySurface.targets) ||
    !capabilitySurface.targets.some((target) => target?.id === recipe.target) ||
    !Array.isArray(capabilitySurface.capabilities)
  ) {
    throw new Error("Capability surface descriptor is invalid.");
  }
  if (stableJson(recipe.capabilities) !== stableJson(NODE_WIRE_CONTRACT.artifact.capability_ids)) {
    throw new Error("The default Node candidate recipe must match the canonical wire capabilities.");
  }
  const capabilities = nodeWireContract(localLayout).artifact.capability_ids;
  const known = new Map(
    capabilitySurface.capabilities.map((capability) => [capability?.id, capability]),
  );
  for (const capabilityId of capabilities) {
    const capability = known.get(capabilityId);
    if (!capability || !Array.isArray(capability.targets) || !capability.targets.includes(recipe.target)) {
      throw new Error(
        `Candidate capability recipe includes ${capabilityId}, which is unavailable on ${recipe.target}.`,
      );
    }
    if (
      !Array.isArray(capability.implications) ||
      capability.implications.some((implication) => !capabilities.includes(implication))
    ) {
      throw new Error(
        `Candidate capability recipe omits an implication of ${capabilityId}.`,
      );
    }
  }
  return capabilities;
}

function candidateDiagramFamilies() {
  const families = sortedUniqueStrings(descriptor.expected?.diagram_families, "Candidate diagram families");
  if (families.some((family) => !/^[a-z][A-Za-z0-9]*$/.test(family) || family === "error")) {
    throw new Error("Candidate diagram families must contain logical family IDs excluding error.");
  }
  return families;
}

export function resolveCandidateRuntimeContract({ localLayout = null } = {}) {
  const capabilityIds = candidateCapabilityFeatures(localLayout);
  const target = descriptor.capability_recipe.target;
  if (
    !Array.isArray(capabilitySurface.outputs) ||
    !Array.isArray(capabilitySurface.binding_operations)
  ) {
    throw new Error("Capability surface descriptor lacks outputs or binding operations.");
  }
  const capabilityById = new Map(
    capabilitySurface.capabilities.map((capability) => [capability?.id, capability]),
  );
  const outputById = new Map(
    capabilitySurface.outputs.map((output) => [output?.id, output]),
  );
  const operations = capabilitySurface.binding_operations
    .filter(
      (operation) =>
        operation?.targets?.includes(target) &&
        NODE_WIRE_CONTRACT.artifact.operation_ids.includes(operation?.id) &&
        (operation.capability === null || capabilityIds.includes(operation.capability)),
    );
  for (const operation of operations) {
    if (
      !Object.hasOwn(operation, "output") ||
      !(operation.output === null || typeof operation.output === "string") ||
      !Array.isArray(operation.compiled_prerequisites) ||
      operation.compiled_prerequisites.some(
        (capability) => typeof capability !== "string" || capability.length === 0,
      ) ||
      stableJson(operation.compiled_prerequisites) !==
        stableJson([...new Set(operation.compiled_prerequisites)].sort())
    ) {
      throw new Error(`Candidate binding operation ${operation?.id ?? "<unknown>"} is invalid.`);
    }
    if (operation.output !== null && !outputById.has(operation.output)) {
      throw new Error(
        `Candidate binding operation ${operation.id} references unknown output ${operation.output}.`,
      );
    }
  }
  const operationIds = operations.map((operation) => operation.id).sort();
  const outputIds = operations
    .map((operation) => operation.output)
    .filter((output) => output !== null)
    .sort();
  const systemAdapterIds = capabilityIds
    .filter((capabilityId) => capabilityById.get(capabilityId)?.kind === "adapter")
    .sort();
  for (const [label, ids] of [
    ["output", outputIds],
    ["operation", operationIds],
  ]) {
    if (
      ids.length === 0 ||
      ids.some((id) => typeof id !== "string" || id.length === 0) ||
      stableJson(ids) !== stableJson([...new Set(ids)].sort())
    ) {
      throw new Error(`Candidate runtime ${label} IDs are invalid.`);
    }
  }
  return {
    capabilityRecipe: {
      descriptor: descriptor.capability_recipe.descriptor,
      target,
      capabilities: capabilityIds,
    },
    capabilityIds,
    diagramFamilies: candidateDiagramFamilies(),
    outputIds,
    operationIds,
    systemAdapterIds,
    textMeasurementProviderIds: ["deterministic"],
  };
}

export function candidateDependencyClosure(metadata, tree, recipe) {
  const packagesByNameVersion = new Map();
  for (const item of metadata.packages) {
    const key = `${item.name} v${item.version}`;
    const matches = packagesByNameVersion.get(key) ?? [];
    matches.push(item);
    packagesByNameVersion.set(key, matches);
  }
  const selected = new Map();
  for (const [index, line] of tree.split(/\r?\n/).entries()) {
    const title = line.match(/^\S+ v\S+/)?.[0];
    const matches = packagesByNameVersion.get(title);
    if (matches?.length !== 1) {
      throw new Error(`Cargo tree package has no unique metadata identity: ${line}.`);
    }
    const { name, version, source, manifest_path: manifest } = matches[0];
    if (index === 0 && name !== cargoPackage) {
      throw new Error(`Cargo tree must start at ${cargoPackage}.`);
    }
    const item = { name, version, source: source ?? localPackageSource(manifest) };
    selected.set(packageIdentity(item), item);
  }
  const packages = [...selected.keys()].sort(comparePackageIdentities).map((key) => selected.get(key));
  validateCandidateDependencyPackages(packages, recipe);
  return {
    digest: digestJson(packages),
    packages,
  };
}

export function resolveCandidateBuildEvidence(recipe) {
  const dependencyClosure = resolveCargoClosure(recipe);
  const inputEntries = collectLocalInputEntries(dependencyClosure);
  return {
    source_digest: digestJson(inputEntries),
    binding_contract_digest: digestJson(collectBindingContractEntries(inputEntries)),
    dependency_closure_digest: dependencyClosure.digest,
  };
}

export function validateCandidateCargoMetadata(recipe) {
  resolveCargoClosure(recipe);
  return packageSurface.version;
}

export function validateCandidatePackageVersions(metadata) {
  if (!metadata || !Array.isArray(metadata.packages)) {
    throw new Error("Cargo metadata lacks candidate package versions.");
  }
  for (const name of [cargoPackage, "merman-bindings-core"]) {
    const matches = metadata.packages.filter((item) => item.name === name);
    if (
      matches.length !== 1 ||
      matches[0].source !== null ||
      matches[0].version !== packageSurface.version
    ) {
      throw new Error(
        `${name} must be the local ${packageSurface.version} package for this Node candidate.`,
      );
    }
  }
  const candidate = metadata.packages.find((item) => item.name === cargoPackage);
  if (
    typeof metadata.workspace_root !== "string" ||
    realpathSync(metadata.workspace_root) !== realpathSync(repositoryRoot) ||
    !Array.isArray(metadata.workspace_members) ||
    !metadata.workspace_members.includes(candidate.id) ||
    path.relative(repositoryRoot, candidate.manifest_path).split(path.sep).join("/") !== descriptor.cargo.manifest
  ) {
    throw new Error(`${cargoPackage} must use the root Cargo workspace and lockfile.`);
  }
  return packageSurface.version;
}

export function validateCandidateDependencyPackages(packages, { candidate, localLayout = null }) {
  if (
    !Array.isArray(packages) ||
    packages.length === 0 ||
    packages.some(
      (item) =>
        !item ||
        typeof item.name !== "string" ||
        item.name.length === 0 ||
        typeof item.version !== "string" ||
        item.version.length === 0 ||
        typeof item.source !== "string" ||
        item.source.length === 0,
    )
  ) {
    throw new Error("Node candidate dependency closure is invalid.");
  }
  const identities = packages.map(packageIdentity);
  const normalizedIdentities = [...identities].sort(comparePackageIdentities);
  if (
    new Set(identities).size !== identities.length ||
    stableJson(identities) !== stableJson(normalizedIdentities)
  ) {
    throw new Error("Node candidate dependency closure packages must be sorted and unique.");
  }
  const names = new Set(packages.map((item) => item.name));
  for (const name of [cargoPackage, "merman-bindings-core"]) {
    const matches = packages.filter((item) => item.name === name);
    if (matches.length !== 1) {
      throw new Error(`${name} is missing or ambiguous in the Node candidate dependency closure.`);
    }
    if (
      matches[0].version !== packageSurface.version ||
      !matches[0].source.startsWith("path:")
    ) {
      throw new Error(
        `${name} must be the local ${packageSurface.version} package in the Node candidate dependency closure.`,
      );
    }
  }
  const shoal = packages.filter((item) => item.name === "merman-layout-shoal");
  if (localLayout === "shoal"
    ? shoal.length !== 1 || shoal[0].source !== "path:crates/merman-layout-shoal"
    : shoal.length !== 0
  ) {
    throw new Error("The Node dependency closure must contain Shoal only for the local Shoal candidate.");
  }
  if (candidate === "node-wasm") {
    for (const forbidden of ["napi", "napi-build", "napi-derive"]) {
      if (names.has(forbidden)) {
        throw new Error(`${forbidden} leaked into the Node WASM dependency closure.`);
      }
    }
    for (const required of ["wasm-bindgen"]) {
      if (!names.has(required)) {
        throw new Error(`${required} is missing from the Node WASM dependency closure.`);
      }
    }
    return;
  }
  if (candidate === "napi") {
    for (const required of ["napi", "napi-build", "napi-derive"]) {
      if (!names.has(required)) {
        throw new Error(`${required} is missing from the napi dependency closure.`);
      }
    }
    return;
  }
  throw new Error(`Unknown candidate dependency closure: ${candidate}.`);
}

function packageIdentity(item) {
  return `${item.name}\0${item.version}\0${item.source}`;
}

function comparePackageIdentities(left, right) {
  if (left < right) return -1;
  if (left > right) return 1;
  return 0;
}

function parseArgs(args) {
  if (args.includes("--help") || args.includes("-h")) {
    console.log("usage: node scripts/build-candidate.mjs --candidate <node-wasm|napi> [--target <target-id>] [--local-grid | --local-layered | --local-shoal]");
    process.exit(0);
  }
  const localLayout = readLocalLayoutFlag(args);
  args = args.filter((arg) => !arg.startsWith("--local-"));
  const candidate = valueAfter(args, "--candidate");
  const target = valueAfter(args, "--target");
  const known = new Set(["--candidate", "--target"]);
  for (let index = 0; index < args.length; index += 2) {
    if (!known.has(args[index]) || args[index + 1] === undefined) {
      throw new Error(`Unknown or incomplete argument: ${args[index] ?? "<missing>"}.`);
    }
  }
  if (!candidate) throw new Error("--candidate is required.");
  if (candidate === "node-wasm" && target !== null) {
    throw new Error("--target applies only to the napi candidate.");
  }
  return { candidate, target, localLayout };
}

function assertDescriptor() {
  if (
    descriptor.schema_version !== 3 ||
    descriptor.status !== "napi-selected-for-alpha" ||
    descriptor.cargo.default_features !== false ||
    stableJson(descriptor.cargo.features) !== stableJson(["all-diagrams"]) ||
    descriptor.cargo.manifest !== "crates/merman-node/Cargo.toml" ||
    descriptor.candidates?.napi?.profile !== "native-sdk" ||
    descriptor.candidates?.["node-wasm"]?.profile !== "release" ||
    descriptor.capability_recipe?.descriptor !== "capabilities/feature-surface-v1.json" ||
    descriptor.capability_recipe?.target !== "native" ||
    packageSurface.schema_version !== 1 ||
    packageSurface.admission_status !== "public-alpha" ||
    typeof packageSurface.version !== "string" ||
    packageSurface.version.length === 0
  ) {
    throw new Error("Node candidate build descriptor is invalid.");
  }
}

function walkFiles(root, { skipBuildOutputs = false } = {}) {
  if (!existsSync(root)) return [];
  const files = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (
      skipBuildOutputs &&
      entry.isDirectory() &&
      new Set([".git", "node_modules", "target", "artifacts", "dist-packages", "reports"]).has(entry.name)
    ) {
      continue;
    }
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) files.push(...walkFiles(absolute, { skipBuildOutputs }));
    else if (entry.isFile()) files.push(absolute);
  }
  return files;
}

function digestFile(file) {
  const digest = createHash("sha256");
  digest.update(readFileSync(file));
  return `sha256:${digest.digest("hex")}`;
}

function localPackageSource(manifestPath) {
  const directory = path.dirname(path.resolve(manifestPath));
  if (!isWithin(repositoryRoot, directory)) {
    throw new Error(`Local Cargo package escapes the repository: ${manifestPath}.`);
  }
  return `path:${path.relative(repositoryRoot, directory).split(path.sep).join("/")}`;
}

function run(command, args, environment = {}) {
  const result = spawnSync(command, args, {
    cwd: nodeRoot,
    env: {
      ...process.env,
      CARGO_BUILD_JOBS: "1",
      CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || path.join(repositoryRoot, "target"),
      ...environment,
    },
    stdio: "inherit",
  });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status ?? 1}.`);
}

function runCapture(command, args) {
  const result = spawnSync(command, args, {
    cwd: nodeRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`${command} exited with status ${result.status ?? 1}: ${(result.stderr ?? "").trim()}`);
  }
  return result.stdout.trim();
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function assertFile(file, label) {
  if (!existsSync(file) || !statSync(file).isFile()) throw new Error(`Missing ${label}: ${file}.`);
}

function valueAfter(args, flag) {
  const index = args.indexOf(flag);
  return index === -1 ? null : args[index + 1] ?? null;
}

function isWithin(root, candidate) {
  const relative = path.relative(root, candidate);
  return relative !== "" && !relative.startsWith("..") && !path.isAbsolute(relative);
}

function isMainModule() {
  return process.argv[1] && realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
}
