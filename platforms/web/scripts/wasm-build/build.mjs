import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

import {
  assertKnownArgs,
  hasHelpFlag,
  parseArgValue,
  resolvePackageSubdir,
} from "./arg-parse.mjs";
import { cleanPackageOutput } from "./clean-package.mjs";
import {
  WASM_INPUT_MANIFEST_NAME,
  buildWasmInputManifest,
  cargoMetadataForPreset,
  cargoRepositoryMetadata,
  collectWasmInputEntries,
  currentWasmBuildToolVersions,
  rustcWasmInputPaths,
} from "./input-manifest.mjs";
import {
  acquireOutputLock,
  acquireWorkspaceWasmBuildLock,
} from "./output-lock.mjs";
import {
  cleanupOutputStage,
  createOutputStage,
  publishStagedOutput,
  recoverOutputTransaction,
} from "./output-transaction.mjs";
import { repositoryRoot, webPackageRoot } from "./paths.mjs";
import {
  defaultWebPackage,
  webPackageDescriptors,
} from "./web-surface-descriptor.mjs";

const packagesById = new Map(webPackageDescriptors.map((item) => [item.id, item]));

export function runBuildWasmCli(args = process.argv.slice(2)) {
  if (hasHelpFlag(args)) {
    printUsage();
    return;
  }

  let selected;
  try {
    selected = parseCli(args);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exitCode = 2;
    return;
  }

  try {
    const repositoryMetadata = cargoRepositoryMetadata({ repoRoot: repositoryRoot });
    const toolVersions = currentWasmBuildToolVersions(repositoryRoot);
    for (const descriptor of selected) {
      buildWasm({
        outputDir: resolvePackageSubdir(
          webPackageRoot,
          `pkg/${descriptor.id}`,
          `package ${descriptor.id} output`,
        ),
        descriptor,
        repositoryMetadata,
        toolVersions,
      });
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode =
      error && typeof error === "object" && "exitCode" in error
        ? Number(error.exitCode) || 1
        : 1;
  }
}

export function buildWasm({
  outputDir,
  descriptor,
  repositoryMetadata = null,
  toolVersions = null,
}) {
  const profile = wasmArtifactProfile(descriptor);
  const outputRoot = outputDir.absolute;
  const releaseLock = acquireOutputLock(outputRoot);
  let stageRoot = null;

  try {
    const lockedRepositoryMetadata =
      repositoryMetadata ?? cargoRepositoryMetadata({ repoRoot: repositoryRoot });
    recoverOutputTransaction(outputRoot);
    stageRoot = createOutputStage(outputRoot);
    console.log(
      [
        `build-wasm: package=${descriptor.name}`,
        `artifact_profile=${profile.name}`,
        `default_features=${profile.default_features}`,
        `features=${profile.features.length > 0 ? profile.features.join("+") : "none"}`,
      ].join(" "),
    );

    const buildMetadata = cargoMetadataForPreset({
      preset: profile,
      repoRoot: repositoryRoot,
      repositoryMetadata: lockedRepositoryMetadata,
    });
    const inputSnapshot = collectWasmInputEntries({ metadata: buildMetadata, repoRoot: repositoryRoot });
    const buildToolVersions = toolVersions ?? currentWasmBuildToolVersions(repositoryRoot);

    const releaseBuildLock = acquireWorkspaceWasmBuildLock(repositoryRoot);
    try {
      run("wasm-pack", wasmPackArgs(profile, stageRoot));
    } finally {
      releaseBuildLock();
    }
    writePackageMetadata(buildMetadata, stageRoot);
    cleanPackageOutput(stageRoot);
    writeArtifactProfileManifest(descriptor, stageRoot);

    const postBuildInputs = collectWasmInputEntries({
      metadata: buildMetadata,
      repoRoot: repositoryRoot,
    });
    if (JSON.stringify(postBuildInputs) !== JSON.stringify(inputSnapshot)) {
      throw new Error("WASM source inputs changed during the build; rerun the same build command.");
    }
    const compilerInputs = rustcWasmInputPaths({
      metadata: lockedRepositoryMetadata,
      repoRoot: repositoryRoot,
    });
    const compiledInputSnapshot = collectWasmInputEntries({
      additionalInputs: compilerInputs,
      metadata: buildMetadata,
      repoRoot: repositoryRoot,
    });
    const manifest = buildWasmInputManifest({
      compilerInputs,
      metadata: buildMetadata,
      outputRoot: stageRoot,
      preset: profile,
      repoRoot: repositoryRoot,
      toolVersions: buildToolVersions,
    });
    if (JSON.stringify(manifest.inputs) !== JSON.stringify(compiledInputSnapshot)) {
      throw new Error(
        "Compiler-reported WASM inputs changed after compilation; rerun the same build command.",
      );
    }
    writeFileSync(
      path.join(stageRoot, WASM_INPUT_MANIFEST_NAME),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );

    publishStagedOutput(stageRoot, outputRoot);
    stageRoot = null;
    console.log(
      `[merman-web] Published ${descriptor.id} WASM transaction (${manifest.input_digest.slice(0, 12)}).`,
    );
  } finally {
    try {
      if (stageRoot) cleanupOutputStage(stageRoot);
    } finally {
      releaseLock();
    }
  }
}

export function wasmArtifactProfile(descriptor) {
  if (!descriptor || typeof descriptor !== "object" || !descriptor.artifact_profile) {
    throw new Error("Web package descriptor is invalid.");
  }
  const profile = descriptor.artifact_profile;
  return {
    name: profile.id,
    surface: "web",
    default_features: profile.cargo.default_features,
    features: [...profile.cargo.features],
    runtime_capability_ids: [...profile.expected.runtime_ids],
    runtime_output_ids: [...profile.expected.outputs],
  };
}

export function wasmArtifactProfileManifest(descriptor) {
  const profile = wasmArtifactProfile(descriptor);
  return {
    schema_version: 1,
    package: descriptor.name,
    package_id: descriptor.id,
    artifact_profile: profile.name,
    default_features: profile.default_features,
    features: profile.features,
    runtime_capability_ids: profile.runtime_capability_ids,
    runtime_output_ids: profile.runtime_output_ids,
  };
}

function parseCli(args) {
  assertKnownArgs(args, {
    valueArgs: ["--package"],
    booleanArgs: ["--all-packages", "--help", "-h"],
  });
  const allPackages = args.includes("--all-packages");
  const id = parseArgValue(args, "--package");
  if (allPackages && id !== null) {
    throw new Error("--all-packages and --package are mutually exclusive.");
  }
  if (allPackages) return webPackageDescriptors;
  const descriptor = packagesById.get(id ?? defaultWebPackage.id);
  if (!descriptor) {
    throw new Error(
      `Unknown @mermanjs browser package '${id}'; expected one of: ${[...packagesById.keys()].join(", ")}.`,
    );
  }
  return [descriptor];
}

function wasmPackArgs(profile, outputRoot) {
  const args = [
    "build",
    "../../crates/merman-wasm",
    "--target",
    "web",
    "--profile",
    "wasm-size",
    "--no-pack",
    "--out-dir",
    outputRoot,
  ];
  const cargoArgs = ["--no-default-features"];
  if (profile.features.length > 0) cargoArgs.push("--features", profile.features.join(","));
  args.push("--", ...cargoArgs);
  return args;
}

function writeArtifactProfileManifest(descriptor, outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  writeFileSync(
    path.join(outputRoot, "merman_wasm_artifact_profile.json"),
    `${JSON.stringify(wasmArtifactProfileManifest(descriptor), null, 2)}\n`,
  );
}

function writePackageMetadata(metadata, outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  const manifestPath = path.resolve(repositoryRoot, "crates", "merman-wasm", "Cargo.toml");
  const packageInfo = metadata.packages.find(
    (item) => path.resolve(item.manifest_path) === manifestPath,
  );
  if (!packageInfo) {
    throw new Error(`Cargo metadata is missing the merman-wasm package: ${manifestPath}`);
  }
  for (const field of ["description", "license", "repository", "homepage"]) {
    if (typeof packageInfo[field] !== "string" || packageInfo[field].length === 0) {
      throw new Error(`Cargo metadata field merman-wasm.${field} is missing.`);
    }
  }
  if (!Array.isArray(packageInfo.authors) || !Array.isArray(packageInfo.keywords)) {
    throw new Error("Cargo metadata fields merman-wasm.authors and keywords must be arrays.");
  }
  const packageJson = {
    name: "merman-wasm-build-artifact",
    type: "module",
    collaborators: packageInfo.authors,
    description: packageInfo.description,
    version: packageInfo.version,
    license: packageInfo.license,
    repository: { type: "git", url: packageInfo.repository },
    files: ["merman_wasm_bg.wasm", "merman_wasm.js", "merman_wasm.d.ts"],
    main: "merman_wasm.js",
    homepage: packageInfo.homepage,
    types: "merman_wasm.d.ts",
    sideEffects: ["./snippets/*"],
    keywords: packageInfo.keywords,
  };
  writeFileSync(path.join(outputRoot, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: webPackageRoot, stdio: "inherit" });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    const error = new Error(`${command} exited with status ${result.status ?? 1}`);
    error.exitCode = result.status ?? 1;
    throw error;
  }
}

function printUsage() {
  console.log("usage: node scripts/build-wasm.mjs [--package <full|analysis|render|editor|ascii> | --all-packages]");
}
