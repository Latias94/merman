import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkgRoot = path.join(packageRoot, "pkg");
const presetManifestPath = path.join(pkgRoot, "merman_wasm_preset.json");

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
    },
  },
  "browser-full-no-elk": {
    surface: "browser",
    defaultFeatures: false,
    features: ["core-full", "core-host", "render", "ascii"],
    capabilities: {
      render: true,
      ascii: true,
      core_full: true,
      core_host: true,
      elk_layout: false,
      ratex_math: false,
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
    },
  },
};

const defaultPresetName = "browser-full";
const presetName =
  parsePreset(process.argv.slice(2)) ??
  process.env.MERMAN_WEB_PRESET ??
  defaultPresetName;
const preset = presets[presetName];

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
  "--target",
  "web",
  "--profile",
  "wasm-size",
  "--out-dir",
  "../../platforms/web/pkg",
  "../../crates/merman-wasm",
];
const cargoArgs = cargoFeatureArgs(preset);

if (cargoArgs.length > 0) {
  wasmPackArgs.push("--", ...cargoArgs);
}

run("wasm-pack", wasmPackArgs);
run(process.execPath, ["scripts/clean-pkg.mjs"]);
writePresetManifest(presetName, preset);

function parsePreset(args) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") {
      printUsage();
      process.exit(0);
    }
    if (arg === "--preset") {
      return args[index + 1];
    }
    if (arg.startsWith("--preset=")) {
      return arg.slice("--preset=".length);
    }
  }
  return null;
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

function writePresetManifest(name, selectedPreset) {
  mkdirSync(pkgRoot, { recursive: true });
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
  console.log("usage: node scripts/build-wasm.mjs [--preset <name>]");
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
