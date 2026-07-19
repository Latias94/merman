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
import { acquireOutputLock } from "./output-lock.mjs";
import {
  cleanupOutputStage,
  createOutputStage,
  publishStagedOutput,
  recoverOutputTransaction,
} from "./output-transaction.mjs";
import { pruneUnownedGeneratedDirectories } from "./package-ownership.mjs";
import { repositoryRoot, webPackageRoot } from "./paths.mjs";
import {
  defaultWebPresetName,
  webPresetDescriptors,
} from "./web-surface-descriptor.mjs";

const presets = new Map(webPresetDescriptors.map((preset) => [preset.name, preset]));

export function runBuildWasmCli(args = process.argv.slice(2)) {
  if (hasHelpFlag(args)) {
    printUsage();
    return;
  }

  let parsed;
  try {
    parsed = parseCli(args);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exitCode = 2;
    return;
  }

  const preset = presets.get(parsed.presetName);
  if (!preset) {
    console.error(`Unknown @mermanjs/web WASM preset: ${parsed.presetName}`);
    printUsage();
    process.exitCode = 2;
    return;
  }

  try {
    buildWasm({ outputDir: parsed.outputDir, preset });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode =
      error && typeof error === "object" && "exitCode" in error
        ? Number(error.exitCode) || 1
        : 1;
  }
}

export function buildWasm({ outputDir, preset }) {
  const outputRoot = outputDir.absolute;
  const rootPackage = normalizePath(outputDir.relative) === "pkg";
  const releaseLock = acquireOutputLock(outputRoot);
  let stageRoot = null;

  try {
    recoverOutputTransaction(outputRoot, { rootPackage });
    if (rootPackage) {
      const removed = pruneUnownedGeneratedDirectories(outputRoot);
      for (const directory of removed) {
        console.log(`[merman-web] Removed unowned generated output: pkg/${directory}`);
      }
    }
    stageRoot = createOutputStage(outputRoot);
    console.log(
      [
        `build-wasm: preset=${preset.name}`,
        `default_features=${preset.default_features}`,
        `features=${preset.features.length > 0 ? preset.features.join("+") : "none"}`,
      ].join(" "),
    );

    const buildMetadata = cargoMetadataForPreset({
      preset,
      repoRoot: repositoryRoot,
    });
    const inputSnapshot = collectWasmInputEntries({
      metadata: buildMetadata,
      repoRoot: repositoryRoot,
    });
    const toolVersions = currentWasmBuildToolVersions(repositoryRoot);

    run("wasm-pack", wasmPackArgs(preset, stageRoot));
    writePackageMetadata(stageRoot);
    cleanPackageOutput(stageRoot);
    writePresetManifest(preset, stageRoot);

    const manifest = buildWasmInputManifest({
      metadata: cargoMetadataForPreset({
        preset,
        repoRoot: repositoryRoot,
      }),
      outputRoot: stageRoot,
      preset,
      repoRoot: repositoryRoot,
      toolVersions,
    });
    if (JSON.stringify(manifest.inputs) !== JSON.stringify(inputSnapshot)) {
      throw new Error(
        "WASM source inputs changed during the build; rerun the same build command.",
      );
    }
    writeFileSync(
      path.join(stageRoot, WASM_INPUT_MANIFEST_NAME),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );

    publishStagedOutput(stageRoot, outputRoot, { rootPackage });
    stageRoot = null;
    console.log(
      `[merman-web] Published ${preset.name} WASM transaction (${manifest.input_digest.slice(0, 12)}).`,
    );
  } finally {
    try {
      if (stageRoot) cleanupOutputStage(stageRoot);
    } finally {
      releaseLock();
    }
  }
}

function parseCli(args) {
  assertKnownArgs(args, {
    valueArgs: ["--preset", "--out-dir-rel"],
    booleanArgs: ["--help", "-h"],
  });
  const outDirRel = parseArgValue(args, "--out-dir-rel") ?? "pkg";
  return {
    presetName:
      parseArgValue(args, "--preset") ??
      process.env.MERMAN_WEB_PRESET ??
      defaultWebPresetName,
    outputDir: resolvePackageSubdir(
      webPackageRoot,
      outDirRel,
      "--out-dir-rel",
    ),
  };
}

function wasmPackArgs(preset, outputRoot) {
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
  const cargoArgs = [];
  if (!preset.default_features) cargoArgs.push("--no-default-features");
  if (preset.features.length > 0) {
    cargoArgs.push("--features", preset.features.join(","));
  }
  if (cargoArgs.length > 0) args.push("--", ...cargoArgs);
  return args;
}

function writePresetManifest(preset, outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  const manifest = {
    schema_version: 1,
    preset: preset.name,
    surface: preset.surface,
    package: "merman-wasm",
    default_features: preset.default_features,
    features: preset.features,
    capabilities: preset.capabilities,
  };
  writeFileSync(
    path.join(outputRoot, "merman_wasm_preset.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

function writePackageMetadata(outputRoot) {
  mkdirSync(outputRoot, { recursive: true });
  const workspaceCargo = readFileSync(path.join(repositoryRoot, "Cargo.toml"), "utf8");
  const wasmCargo = readFileSync(
    path.join(repositoryRoot, "crates", "merman-wasm", "Cargo.toml"),
    "utf8",
  );
  const packageJson = {
    name: "merman-wasm",
    type: "module",
    collaborators: tomlStringArray(workspaceCargo, "authors"),
    description: tomlString(wasmCargo, "description"),
    version: tomlString(workspaceCargo, "version"),
    license: tomlString(workspaceCargo, "license"),
    repository: {
      type: "git",
      url: tomlString(workspaceCargo, "repository"),
    },
    files: ["merman_wasm_bg.wasm", "merman_wasm.js", "merman_wasm.d.ts"],
    main: "merman_wasm.js",
    homepage: tomlString(workspaceCargo, "homepage"),
    types: "merman_wasm.d.ts",
    sideEffects: ["./snippets/*"],
    keywords: tomlStringArray(wasmCargo, "keywords"),
  };
  writeFileSync(
    path.join(outputRoot, "package.json"),
    `${JSON.stringify(packageJson, null, 2)}\n`,
  );
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
  const result = spawnSync(command, args, {
    cwd: webPackageRoot,
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
  console.log("usage: node scripts/build-wasm.mjs [--preset <name>] [--out-dir-rel <dir>]");
  console.log();
  console.log("Presets:");
  for (const preset of webPresetDescriptors) {
    console.log(
      [
        `  ${preset.name.padEnd(20)}`,
        `default_features=${preset.default_features}`,
        `features=${preset.features.length > 0 ? preset.features.join("+") : "none"}`,
      ].join(" "),
    );
  }
}

function normalizePath(value) {
  return value.split(path.sep).join("/");
}
