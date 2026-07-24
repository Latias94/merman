import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
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
  collectWasmInputEntries,
  currentWasmBuildToolVersions,
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
    for (const descriptor of selected) {
      buildWasm({
        outputDir: resolvePackageSubdir(
          webPackageRoot,
          `pkg/${descriptor.id}`,
          `package ${descriptor.id} output`,
        ),
        descriptor,
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

export function buildWasm({ outputDir, descriptor }) {
  const profile = wasmArtifactProfile(descriptor);
  const outputRoot = outputDir.absolute;
  const releaseLock = acquireOutputLock(outputRoot);
  let stageRoot = null;

  try {
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

    const buildMetadata = cargoMetadataForPreset({ preset: profile, repoRoot: repositoryRoot });
    const inputSnapshot = collectWasmInputEntries({ metadata: buildMetadata, repoRoot: repositoryRoot });
    const toolVersions = currentWasmBuildToolVersions(repositoryRoot);

    const releaseBuildLock = acquireWorkspaceWasmBuildLock(repositoryRoot);
    try {
      run("wasm-pack", wasmPackArgs(profile, stageRoot));
    } finally {
      releaseBuildLock();
    }
    writePackageMetadata(stageRoot);
    cleanPackageOutput(stageRoot);
    writeArtifactProfileManifest(descriptor, stageRoot);

    const manifest = buildWasmInputManifest({
      metadata: cargoMetadataForPreset({ preset: profile, repoRoot: repositoryRoot }),
      outputRoot: stageRoot,
      preset: profile,
      repoRoot: repositoryRoot,
      toolVersions,
    });
    if (JSON.stringify(manifest.inputs) !== JSON.stringify(inputSnapshot)) {
      throw new Error("WASM source inputs changed during the build; rerun the same build command.");
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
  const profile = wasmArtifactProfile(descriptor);
  writeFileSync(
    path.join(outputRoot, "merman_wasm_artifact_profile.json"),
    `${JSON.stringify({
      schema_version: 1,
      package: descriptor.name,
      package_id: descriptor.id,
      artifact_profile: profile.name,
      default_features: profile.default_features,
      features: profile.features,
      runtime_capability_ids: profile.runtime_capability_ids,
    }, null, 2)}\n`,
  );
}

function writePackageMetadata(outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  const workspaceCargo = readFileSync(path.join(repositoryRoot, "Cargo.toml"), "utf8");
  const wasmCargo = readFileSync(path.join(repositoryRoot, "crates", "merman-wasm", "Cargo.toml"), "utf8");
  const packageJson = {
    name: "merman-wasm-build-artifact",
    type: "module",
    collaborators: tomlStringArray(workspaceCargo, "authors"),
    description: tomlString(wasmCargo, "description"),
    version: tomlString(workspaceCargo, "version"),
    license: tomlString(workspaceCargo, "license"),
    repository: { type: "git", url: tomlString(workspaceCargo, "repository") },
    files: ["merman_wasm_bg.wasm", "merman_wasm.js", "merman_wasm.d.ts"],
    main: "merman_wasm.js",
    homepage: tomlString(workspaceCargo, "homepage"),
    types: "merman_wasm.d.ts",
    sideEffects: ["./snippets/*"],
    keywords: tomlStringArray(wasmCargo, "keywords"),
  };
  writeFileSync(path.join(outputRoot, "package.json"), `${JSON.stringify(packageJson, null, 2)}\n`);
}

function tomlString(source, key) {
  const match = source.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m"));
  if (!match) throw new Error(`Missing TOML string field: ${key}`);
  return match[1];
}

function tomlStringArray(source, key) {
  const match = source.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, "m"));
  if (!match) throw new Error(`Missing TOML string array field: ${key}`);
  return [...match[1].matchAll(/"([^"]*)"/g)].map((item) => item[1]);
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
