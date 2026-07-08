import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  assertKnownArgs,
  hasHelpFlag,
  parseArgValue,
  resolvePackageSubdir,
} from "./arg-parse.mjs";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = path.join(packageRoot, "..", "..");

const presets = {
  "browser-core": {
    surface: "browser",
    defaultFeatures: false,
    features: [],
    capabilities: {
      render: false,
      ascii: false,
      core_full: false,
      core_host: false,
      elk_layout: false,
      ratex_math: false,
      editor_language: false,
    },
  },
  "browser-render": {
    surface: "browser",
    defaultFeatures: false,
    features: ["render"],
    capabilities: {
      render: true,
      ascii: false,
      core_full: false,
      core_host: false,
      elk_layout: false,
      ratex_math: false,
      editor_language: false,
    },
  },
  "browser-ascii": {
    surface: "browser",
    defaultFeatures: false,
    features: ["ascii"],
    capabilities: {
      render: false,
      ascii: true,
      core_full: true,
      core_host: true,
      elk_layout: false,
      ratex_math: false,
      editor_language: false,
    },
  },
  "browser-full": {
    surface: "browser",
    defaultFeatures: true,
    features: [],
    capabilities: {
      render: true,
      ascii: true,
      core_full: true,
      core_host: true,
      elk_layout: true,
      ratex_math: false,
      editor_language: true,
    },
  },
  "browser-full-no-elk": {
    surface: "browser",
    defaultFeatures: false,
    features: ["core-full", "core-host", "render", "ascii", "editor-language"],
    capabilities: {
      render: true,
      ascii: true,
      core_full: true,
      core_host: true,
      elk_layout: false,
      ratex_math: false,
      editor_language: true,
    },
  },
  "browser-ratex-math": {
    surface: "browser",
    defaultFeatures: true,
    features: ["ratex-math"],
    capabilities: {
      render: true,
      ascii: true,
      core_full: true,
      core_host: true,
      elk_layout: true,
      ratex_math: true,
      editor_language: true,
    },
  },
};

const defaultPresetName = "browser-full";
const args = process.argv.slice(2);
const { presetName, outputDir } = parseCli(args);
const preset = presets[presetName];
const outputRoot = outputDir.absolute;
const presetManifestPath = path.join(outputRoot, "merman_wasm_preset.json");

if (!preset) {
  console.error(`Unknown @mermanjs/web WASM preset: ${presetName}`);
  printUsage();
  process.exit(2);
}

console.log(
  [
    `build-wasm: preset=${presetName}`,
    `default_features=${preset.defaultFeatures}`,
    `features=${preset.features.length > 0 ? preset.features.join("+") : "none"}`,
  ].join(" ")
);

const wasmPackArgs = [
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
const cargoArgs = cargoFeatureArgs(preset);

if (cargoArgs.length > 0) {
  wasmPackArgs.push("--", ...cargoArgs);
}

run("wasm-pack", wasmPackArgs);
writePackageMetadata(outputRoot);
run(process.execPath, ["scripts/clean-pkg.mjs", "--pkg-dir-rel", outputDir.relative]);
writePresetManifest(presetName, preset, outputRoot);

function parseCli(inputArgs) {
  if (hasHelpFlag(inputArgs)) {
    printUsage();
    process.exit(0);
  }
  try {
    assertKnownArgs(inputArgs, {
      valueArgs: ["--preset", "--out-dir-rel"],
      booleanArgs: ["--help", "-h"],
    });
    const outDirRel = parseArgValue(inputArgs, "--out-dir-rel") ?? "pkg";
    return {
      presetName:
        parseArgValue(inputArgs, "--preset") ??
        process.env.MERMAN_WEB_PRESET ??
        defaultPresetName,
      outputDir: resolvePackageSubdir(packageRoot, outDirRel, "--out-dir-rel"),
    };
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exit(2);
  }
}

function cargoFeatureArgs(selectedPreset) {
  const args = [];
  if (!selectedPreset.defaultFeatures) {
    args.push("--no-default-features");
  }
  if (selectedPreset.features.length > 0) {
    args.push("--features", selectedPreset.features.join(","));
  }
  return args;
}

function writePresetManifest(name, selectedPreset, outDir) {
  mkdirSync(outDir, { recursive: true });
  const manifest = {
    schema_version: 1,
    preset: name,
    surface: selectedPreset.surface,
    package: "merman-wasm",
    default_features: selectedPreset.defaultFeatures,
    features: selectedPreset.features,
    capabilities: selectedPreset.capabilities,
  };
  writeFileSync(presetManifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
}

function writePackageMetadata(outDir) {
  mkdirSync(outDir, { recursive: true });

  const workspaceCargo = readFileSync(path.join(repoRoot, "Cargo.toml"), "utf8");
  const wasmCargo = readFileSync(
    path.join(repoRoot, "crates", "merman-wasm", "Cargo.toml"),
    "utf8"
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
    path.join(outDir, "package.json"),
    `${JSON.stringify(packageJson, null, 2)}\n`
  );
}

function tomlString(source, key) {
  const match = source.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, "m"));
  if (!match) {
    throw new Error(`Missing TOML string field: ${key}`);
  }
  return match[1];
}

function tomlStringArray(source, key) {
  const match = source.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, "m"));
  if (!match) {
    throw new Error(`Missing TOML string array field: ${key}`);
  }
  return [...match[1].matchAll(/"([^"]*)"/g)].map((item) => item[1]);
}

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: packageRoot,
    stdio: "inherit",
  });

  if (result.error) {
    console.error(`Failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function printUsage() {
  console.log("usage: node scripts/build-wasm.mjs [--preset <name>] [--out-dir-rel <dir>]");
  console.log();
  console.log("Presets:");
  for (const [name, selectedPreset] of Object.entries(presets)) {
    console.log(
      [
        `  ${name.padEnd(20)}`,
        `default_features=${selectedPreset.defaultFeatures}`,
        `features=${selectedPreset.features.length > 0 ? selectedPreset.features.join("+") : "none"}`,
      ].join(" ")
    );
  }
}
