import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  assembleNativePackages,
  projectLegalMaterial,
} from "../assemble-packages.mjs";
import { digestJson } from "../stable-json.mjs";
import { svgTransportEvidence } from "./svg-signature.mjs";
import {
  assertSuccessfulNpmSpawn,
  spawnNpmSync,
} from "../../../../scripts/npm-command.mjs";

const nodeRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const packageSurface = JSON.parse(
  readFileSync(path.join(nodeRoot, "package-surfaces.json"), "utf8"),
);
const packageVersion = packageSurface.version;
export function measureFootprint({ candidate, artifact, target }) {
  return withCandidateInstallation(
    { candidate, artifact, target },
    ({ footprint }) => footprint,
  );
}

export function withCandidateInstallation(
  { candidate, artifact, target },
  inspect,
) {
  if (typeof inspect !== "function") {
    throw new TypeError("candidate installation inspection callback is required");
  }
  const temporaryRoot = mkdtempSync(path.join(os.tmpdir(), `merman-${candidate}-footprint-`));
  try {
    const packageRoots = candidate === "napi"
      ? stageNativePackages(temporaryRoot, target, artifact)
      : [stageWasmPackage(temporaryRoot, artifact)];
    return inspect(packAndInstall(temporaryRoot, packageRoots, candidate));
  } finally {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

function stageNativePackages(temporaryRoot, target, artifact) {
  const packagesRoot = path.join(temporaryRoot, "packages");
  assembleNativePackages(target, packagesRoot, artifact);
  return [path.join(packagesRoot, "node"), path.join(packagesRoot, target)];
}

export function stageWasmPackage(temporaryRoot, artifact) {
  const packageRoot = path.join(temporaryRoot, "wasm-package");
  mkdirSync(path.join(packageRoot, "artifact"), { recursive: true });
  cpSync(path.dirname(artifact), path.join(packageRoot, "artifact"), { recursive: true });
  cpSync(path.join(nodeRoot, "src"), path.join(packageRoot, "src"), { recursive: true });
  // wasm-pack writes a wildcard .gitignore for its output directory. npm applies nested ignore
  // files while packing, so retaining it would silently remove the candidate loader and WASM.
  rmSync(path.join(packageRoot, "artifact", ".gitignore"), { force: true });
  writeFileSync(
    path.join(packageRoot, "package.json"),
    `${JSON.stringify({
      name: "@mermanjs/node-wasm-candidate",
      version: packageVersion,
      private: true,
      type: "module",
      engines: { node: packageSurface.node_engine },
      main: "./index.mjs",
      types: "./index.d.ts",
      exports: {
        ".": {
          types: "./index.d.ts",
          import: "./index.mjs",
        },
      },
      files: [
        "artifact",
        "index.d.ts",
        "index.mjs",
        "src",
        "LICENSE-APACHE",
        "LICENSE-MIT",
        "THIRD_PARTY_LICENSES",
        "THIRD_PARTY_NOTICES.md",
      ],
      license: "MIT OR Apache-2.0",
    }, null, 2)}\n`,
  );
  writeFileSync(
    path.join(packageRoot, "index.mjs"),
    [
      'import { MermanEngine, createNodeEngine as createEngineWithTransport } from "./src/engine.mjs";',
      'import { loadNodeWasmTransport } from "./src/candidates/wasm.mjs";',
      'const modulePath = new URL("./artifact/merman_node.js", import.meta.url).href;',
      "export { MermanEngine };",
      "export function createNodeEngine(options) {",
      "  return createEngineWithTransport(options, {",
      "    loadTransport: (optionsJson) => loadNodeWasmTransport(optionsJson, { modulePath }),",
      "  });",
      "}",
      "export {",
      "  MermanDisposedError,",
      "  MermanError,",
      "  MermanInvalidTransportError,",
      "  MermanLifecycleError,",
      "  MermanMissingPlatformPackageError,",
      "  MermanOperationError,",
      "  MermanQueueSaturatedError,",
      "  MermanUnsupportedTargetError,",
      '} from "./src/errors.mjs";',
      "",
    ].join("\n"),
  );
  cpSync(path.join(nodeRoot, "src", "index.d.ts"), path.join(packageRoot, "index.d.ts"));
  projectLegalMaterial(packageRoot);
  return packageRoot;
}

function packAndInstall(temporaryRoot, packageRoots, candidate) {
  const tarRoot = path.join(temporaryRoot, "tarballs");
  mkdirSync(tarRoot, { recursive: true });
  const packed = packageRoots.map((packageRoot) => npmPack(packageRoot, tarRoot));
  const installRoot = path.join(temporaryRoot, "install");
  mkdirSync(installRoot, { recursive: true });
  const installManifest = {
    name: "merman-node-footprint-probe",
    private: true,
    version: "0.0.0",
    dependencies: {},
  };
  if (candidate === "napi") {
    const [rootPackage, targetPackage] = packed;
    installManifest.dependencies[rootPackage.name] = fileReference(
      installRoot,
      path.join(tarRoot, rootPackage.filename),
    );
    installManifest.overrides = {
      [targetPackage.name]: fileReference(
        installRoot,
        path.join(tarRoot, targetPackage.filename),
      ),
    };
  } else {
    const [wasmPackage] = packed;
    installManifest.dependencies[wasmPackage.name] = fileReference(
      installRoot,
      path.join(tarRoot, wasmPackage.filename),
    );
  }
  writeFileSync(
    path.join(installRoot, "package.json"),
    `${JSON.stringify(installManifest, null, 2)}\n`,
  );
  runNpm([
    "install",
    "--ignore-scripts",
    "--no-audit",
    "--no-fund",
  ], installRoot);
  const installedRoot = path.join(installRoot, "node_modules");
  const installedFiles = treeEntries(installedRoot);
  const topology = verifyInstalledTopology(installedRoot, packed, candidate);
  const runtime = probeInstalledRuntime(installRoot, candidate);
  const nativePackagePair = candidate === "napi";
  return {
    productModule: installedProductModule(installedRoot, candidate),
    footprint: {
      packed_bytes: packed.reduce((sum, item) => sum + item.size, 0),
      unpacked_bytes: packed.reduce((sum, item) => sum + item.unpacked_size, 0),
      installed_bytes: treeSize(installedRoot),
      runtime_api_passed: runtime.product_entrypoint_passed,
      runtime_catalog_passed: runtime.runtime_catalog_passed,
      generic_operation_passed: runtime.generic_operation_passed,
      svg_plan_operation_passed: runtime.svg_plan_operation_passed,
      svg_operation_passed: runtime.svg_operation_passed,
      request_options_passed: runtime.request_options_passed,
      browser_fallback_absent: topology.browserFallbackAbsent,
      optional_platform_package_passed: topology.optionalPlatformPackagePassed,
      install_method: nativePackagePair ? "root-optional-dependency" : "single-package",
      target_install_passed: true,
      package_count: packed.length,
      packages: packed,
      installed_files: installedFiles,
      installation_evidence: {
        ...topology.installationEvidence,
        install_manifest: installManifest,
        package_lock: readJson(path.join(installRoot, "package-lock.json")),
      },
      runtime_probe: runtime.evidence,
    },
  };
}

function npmPack(packageRoot, tarRoot) {
  const result = spawnNpmSync(["pack", "--json", "--pack-destination", tarRoot], {
    cwd: packageRoot,
    encoding: "utf8",
  });
  assertSuccessfulNpmSpawn(result, "npm pack for footprint measurement");
  const output = JSON.parse(result.stdout)[0];
  if (
    typeof output?.name !== "string" ||
    output.name.length === 0 ||
    typeof output.version !== "string" ||
    output.version.length === 0 ||
    !output.filename ||
    !Number.isFinite(output.size) ||
    !Number.isFinite(output.unpackedSize)
  ) {
    throw new Error("npm pack returned an incomplete footprint result.");
  }
  if (!Array.isArray(output.files)) throw new Error("npm pack returned no package contents.");
  return {
    name: output.name,
    version: output.version,
    filename: output.filename,
    size: output.size,
    unpacked_size: output.unpackedSize,
    files: output.files.map((file) => ({ path: file.path, bytes: file.size })),
  };
}

function treeSize(root) {
  let bytes = 0;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const absolute = path.join(root, entry.name);
    if (entry.isDirectory()) bytes += treeSize(absolute);
    else if (entry.isFile()) bytes += statSync(absolute).size;
  }
  return bytes;
}

function treeEntries(root, current = root) {
  const files = [];
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const absolute = path.join(current, entry.name);
    if (entry.isDirectory()) files.push(...treeEntries(root, absolute));
    else if (entry.isFile()) {
      files.push({
        path: path.relative(root, absolute).split(path.sep).join("/"),
        bytes: statSync(absolute).size,
      });
    }
  }
  return files.sort((left, right) => left.path.localeCompare(right.path));
}

function probeInstalledRuntime(installRoot, candidate) {
  const packageName = candidate === "napi"
    ? "@mermanjs/node"
    : candidate === "node-wasm"
      ? "@mermanjs/node-wasm-candidate"
      : null;
  if (packageName === null) throw new Error(`Unknown footprint candidate: ${candidate}.`);
  const script = [
    `import { createNodeEngine } from ${JSON.stringify(packageName)};`,
    'const engine = await createNodeEngine({ bindingOptions: { version: 2, runtime_policy: "deterministic", resources: { profile: "trusted-native" } } });',
    'const catalog = engine.runtimeCatalog;',
    'const runtimeCatalogPassed = catalog?.capabilities?.operation_ids?.includes("semantic-json") && catalog?.capabilities?.operation_ids?.includes("svg-plan-json") && catalog?.capabilities?.text_measurement?.provider_ids?.join(",") === "vendored";',
    'const semantic = await engine.executeOperation({ operationId: "semantic-json", source: "flowchart TD\\nA-->B", optionsJson: JSON.stringify({ version: 2 }) });',
    'const genericOperationPassed = semantic.operation_id === "semantic-json" && semantic.media_type === "application/json" && JSON.parse(semantic.data);',
    'const svgPlan = await engine.executeOperation({ operationId: "svg-plan-json", source: "flowchart TD\\nA-->B", optionsJson: JSON.stringify({ version: 2 }) });',
    'const parsedSvgPlan = JSON.parse(svgPlan.data);',
    'const svgPlanOperationPassed = svgPlan.operation_id === "svg-plan-json" && svgPlan.media_type === "application/json" && parsedSvgPlan?.schema_version === 1 && parsedSvgPlan?.planned_operation_id === "svg" && Array.isArray(parsedSvgPlan?.required_capability_ids) && Array.isArray(parsedSvgPlan?.missing_capability_ids) && typeof parsedSvgPlan?.ready === "boolean";',
    'const svg = await engine.executeOperation({ operationId: "svg", source: "flowchart TD\\nA-->B", optionsJson: JSON.stringify({ version: 2 }) });',
    'let requestOptionsPassed = false;',
    'let requestOptionsError = null;',
    'try {',
    '  await engine.renderSvg("flowchart TD\\nA-->B", { optionsJson: JSON.stringify({ version: 2, resources: { limits: { max_source_bytes: 4 } } }) });',
    '} catch (error) {',
    '  requestOptionsPassed = error?.codeName === "MERMAN_RESOURCE_LIMIT_EXCEEDED";',
    '  requestOptionsError = { code_name: error?.codeName ?? error?.code ?? null, kind: error?.kind ?? null, capability_id: error?.capabilityId ?? null };',
    '}',
    'await engine.dispose();',
    'process.stdout.write(JSON.stringify({ catalog, semantic, svg_plan: svgPlan, parsed_svg_plan: parsedSvgPlan, svg, request_options_error: requestOptionsError, product_entrypoint_passed: true, runtime_catalog_passed: runtimeCatalogPassed === true, generic_operation_passed: Boolean(genericOperationPassed), svg_plan_operation_passed: svgPlanOperationPassed, request_options_passed: requestOptionsPassed }));',
  ].join("\n");
  const output = runCapture(
    process.execPath,
    ["--input-type=module", "--eval", script],
    installRoot,
  );
  const raw = JSON.parse(output);
  const svgEvidence = svgTransportEvidence(raw.svg?.data);
  const svgOperationPassed =
    raw.svg?.operation_id === "svg" && raw.svg?.media_type === "image/svg+xml";
  const evidence = {
    runtime_catalog_digest: digestJson(raw.catalog),
    semantic_operation: {
      operation_id: raw.semantic?.operation_id,
      media_type: raw.semantic?.media_type,
      result_digest: digestJson(JSON.parse(raw.semantic?.data)),
      bytes: Buffer.byteLength(raw.semantic?.data ?? ""),
    },
    svg_plan_operation: {
      operation_id: raw.svg_plan?.operation_id,
      media_type: raw.svg_plan?.media_type,
      result_digest: digestJson(raw.parsed_svg_plan),
      planned_operation_id: raw.parsed_svg_plan?.planned_operation_id,
      ready: raw.parsed_svg_plan?.ready,
      bytes: Buffer.byteLength(raw.svg_plan?.data ?? ""),
    },
    svg_operation: {
      operation_id: raw.svg?.operation_id,
      media_type: raw.svg?.media_type,
      output_digest: digest(raw.svg?.data ?? ""),
      structure_sha256: svgEvidence.structure_sha256,
      geometry_sha256: svgEvidence.geometry_sha256,
      bytes: Buffer.byteLength(raw.svg?.data ?? ""),
    },
    request_options_error: raw.request_options_error,
  };
  for (const key of [
    "product_entrypoint_passed",
    "runtime_catalog_passed",
    "generic_operation_passed",
    "svg_plan_operation_passed",
    "request_options_passed",
  ]) {
    if (raw[key] !== true) {
      throw new Error(`${candidate} installed product probe failed ${key}.`);
    }
  }
  if (!svgOperationPassed) {
    throw new Error(`${candidate} installed product probe failed svg_operation_passed.`);
  }
  return {
    product_entrypoint_passed: true,
    runtime_catalog_passed: true,
    generic_operation_passed: true,
    svg_plan_operation_passed: true,
    svg_operation_passed: true,
    request_options_passed: true,
    evidence,
  };
}

function verifyInstalledTopology(installedRoot, packed, candidate) {
  const installedFiles = treeEntries(installedRoot);
  const browserFallbackAbsent = installedFiles.every(
    (file) => !file.path.startsWith("@mermanjs/web"),
  );
  if (!browserFallbackAbsent) {
    throw new Error(`${candidate} installed a browser package fallback.`);
  }
  if (candidate === "node-wasm") {
    const [rootPackage] = packed;
    const rootPath = rootPackage.name.split("/");
    const rootManifest = readJson(path.join(installedRoot, ...rootPath, "package.json"));
    return {
      browserFallbackAbsent,
      optionalPlatformPackagePassed: null,
      installationEvidence: {
        root_package: packageEvidence(rootPackage, rootManifest),
        target_package: null,
        product_entrypoint: [...rootPath, "index.mjs"].join("/"),
        loaded_artifacts: [
          fileEvidence(installedRoot, [...rootPath, "artifact", "merman_node.js"]),
          fileEvidence(installedRoot, [...rootPath, "artifact", "merman_node_bg.wasm"]),
          fileEvidence(installedRoot, [...rootPath, "artifact", "package.json"]),
        ],
      },
    };
  }
  if (candidate !== "napi") throw new Error(`Unknown footprint candidate: ${candidate}.`);
  const [rootPackage, targetPackage] = packed;
  const rootManifest = readJson(
    path.join(installedRoot, ...rootPackage.name.split("/"), "package.json"),
  );
  const targetRoot = path.join(installedRoot, ...targetPackage.name.split("/"));
  const targetManifest = readJson(path.join(targetRoot, "package.json"));
  if (
    rootManifest.optionalDependencies?.[targetPackage.name] !== targetManifest.version ||
    !existsSync(path.join(targetRoot, "merman.node"))
  ) {
    throw new Error("napi footprint did not resolve the target package through the root optional dependency.");
  }
  return {
    browserFallbackAbsent,
    optionalPlatformPackagePassed: true,
    installationEvidence: {
      root_package: packageEvidence(rootPackage, rootManifest),
      target_package: packageEvidence(targetPackage, targetManifest),
      product_entrypoint: [
        ...rootPackage.name.split("/"),
        "dist",
        "index.mjs",
      ].join("/"),
      loaded_artifacts: [
        fileEvidence(installedRoot, [...targetPackage.name.split("/"), "merman.node"]),
      ],
    },
  };
}

function packageEvidence(packed, manifest) {
  return {
    name: packed.name,
    version: packed.version,
    manifest,
  };
}

function fileEvidence(root, segments) {
  const absolute = path.join(root, ...segments);
  if (!existsSync(absolute) || !statSync(absolute).isFile()) {
    throw new Error(`Missing installed runtime artifact: ${segments.join("/")}.`);
  }
  return {
    path: segments.join("/"),
    bytes: statSync(absolute).size,
    sha256: digest(readFileSync(absolute)),
  };
}

function installedProductModule(installedRoot, candidate) {
  const relative = candidate === "napi"
    ? ["@mermanjs", "node", "dist", "index.mjs"]
    : candidate === "node-wasm"
      ? ["@mermanjs", "node-wasm-candidate", "index.mjs"]
      : null;
  if (relative === null) throw new Error(`Unknown footprint candidate: ${candidate}.`);
  const modulePath = path.join(installedRoot, ...relative);
  if (!existsSync(modulePath)) throw new Error(`Missing installed product entrypoint: ${modulePath}.`);
  return pathToFileURL(modulePath).href;
}

function runCapture(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.error || result.status !== 0) {
    throw new Error(`${command} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

function runNpm(args, cwd) {
  const result = spawnNpmSync(args, { cwd, encoding: "utf8" });
  assertSuccessfulNpmSpawn(
    result,
    `npm ${args[0] ?? "command"} for footprint measurement`,
  );
}

function fileReference(from, file) {
  return `file:${path.relative(from, file).split(path.sep).join("/")}`;
}

function readJson(file) {
  return JSON.parse(readFileSync(file, "utf8"));
}

function digest(value) {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}
