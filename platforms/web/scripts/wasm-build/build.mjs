import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";

import { hasHelpFlag } from "./arg-parse.mjs";
import { cleanPackageOutput } from "./clean-package.mjs";
import {
  WASM_ARTIFACT_PROFILE_NAME,
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
  parseWasmTargets,
  wasmArtifactProfile,
  wasmArtifactProfileManifest,
} from "./targets.mjs";

export { wasmArtifactProfile, wasmArtifactProfileManifest };

export function runBuildWasmCli(args = process.argv.slice(2)) {
  if (hasHelpFlag(args)) {
    printUsage();
    return;
  }

  let selected;
  try {
    selected = parseWasmTargets(args);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exitCode = 2;
    return;
  }

  try {
    const repositoryMetadata = cargoRepositoryMetadata({ repoRoot: repositoryRoot });
    const toolVersions = currentWasmBuildToolVersions(repositoryRoot);
    for (const target of selected) {
      buildWasm({ ...target, repositoryMetadata, toolVersions });
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
  profile = wasmArtifactProfile(descriptor),
  repositoryMetadata = null,
  toolVersions = null,
}) {
  const outputRoot = outputDir.absolute;
  const outputName = outputDir.relative ?? descriptor.id;
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
        `output=${outputName}`,
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
      run("wasm-pack", wasmPackArgs(profile, stageRoot), {
        env: wasmPackEnvironment(lockedRepositoryMetadata),
      });
    } finally {
      releaseBuildLock();
    }
    writePackageMetadata(buildMetadata, stageRoot);
    cleanPackageOutput(stageRoot);
    writeFileSync(
      path.join(stageRoot, WASM_ARTIFACT_PROFILE_NAME),
      `${JSON.stringify(wasmArtifactProfileManifest(descriptor, profile), null, 2)}\n`,
    );

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
      `[merman-web] Published ${outputName} WASM transaction (${manifest.input_digest.slice(0, 12)}).`,
    );
  } finally {
    try {
      if (stageRoot) cleanupOutputStage(stageRoot);
    } finally {
      releaseLock();
    }
  }
}

export function wasmPackEnvironment(metadata, environment = process.env) {
  if (typeof metadata?.target_directory !== "string" || metadata.target_directory.length === 0) {
    throw new Error("cargo metadata is missing target_directory.");
  }
  return {
    ...environment,
    CARGO_TARGET_DIR: path.resolve(metadata.target_directory),
  };
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

function writePackageMetadata(metadata, outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  const manifestPath = path.resolve(repositoryRoot, "crates", "merman-wasm", "Cargo.toml");
  const packageInfo = metadata.packages.find(
    (item) => path.resolve(item.manifest_path) === manifestPath,
  );
  if (!packageInfo) {
    throw new Error(`Cargo metadata is missing the merman-wasm package: ${manifestPath}`);
  }
  for (const field of ["description", "license", "repository"]) {
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
    homepage:
      typeof packageInfo.homepage === "string" && packageInfo.homepage.length > 0
        ? packageInfo.homepage
        : packageInfo.repository,
    types: "merman_wasm.d.ts",
    sideEffects: ["./snippets/*"],
    keywords: packageInfo.keywords,
  };
  writeFileSync(path.join(outputRoot, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);
}

function run(command, args, { env = process.env } = {}) {
  const result = spawnSync(command, args, {
    cwd: webPackageRoot,
    env,
    stdio: "inherit",
  });
  if (result.error) throw new Error(`Failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    const error = new Error(`${command} exited with status ${result.status ?? 1}`);
    error.exitCode = result.status ?? 1;
    throw error;
  }
}

function printUsage() {
  console.log("usage: node scripts/build-wasm.mjs [--package <full|analysis|render|editor|ascii> | --all-packages] [--local-shoal]");
}
