import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

import {
  assertKnownArgs,
  hasHelpFlag,
  parseArgValue,
  resolvePackageSubdir,
} from "./arg-parse.mjs";
import {
  WASM_INPUT_MANIFEST_NAME,
  cargoMetadataForPreset,
  currentWasmBuildToolVersions,
  verifyWasmInputManifest,
} from "./input-manifest.mjs";
import { publicSurfaceDirectoryNames } from "./package-ownership.mjs";
import { repositoryRoot, webPackageRoot } from "./paths.mjs";
import {
  defaultWebPresetName,
  publicWebSurfaceDescriptors,
  webPresetDescriptors,
} from "./web-surface-descriptor.mjs";

const presets = new Map(webPresetDescriptors.map((preset) => [preset.name, preset]));
const publicSurfaces = new Map(
  publicWebSurfaceDescriptors.map((surface) => [surface.entry, surface]),
);

export function runVerifyWasmInputsCli(args = process.argv.slice(2)) {
  if (hasHelpFlag(args)) {
    printUsage();
    return;
  }

  let targets;
  try {
    targets = parseVerificationTargets(args);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    printUsage();
    process.exitCode = 2;
    return;
  }

  const metadataByPreset = new Map();
  let toolVersions;
  const failures = [];
  for (const target of targets) {
    const checked = verifyTarget(target, {
      getMetadata(preset) {
        if (!metadataByPreset.has(preset.name)) {
          metadataByPreset.set(
            preset.name,
            cargoMetadataForPreset({ preset, repoRoot: repositoryRoot }),
          );
        }
        return metadataByPreset.get(preset.name);
      },
      getToolVersions() {
        toolVersions ??= currentWasmBuildToolVersions(repositoryRoot);
        return toolVersions;
      },
    });
    if (checked.ok) {
      console.log(
        `[merman-web] WASM inputs verified (${target.presetName}, ${target.outputDir.relative}, ${checked.digest.slice(0, 12)}).`,
      );
    } else {
      failures.push({ target, reasons: checked.reasons });
    }
  }

  if (failures.length === 0) return;
  console.error("[merman-web] WASM artifact is stale or unverifiable.");
  for (const failure of failures) {
    console.error(
      `  ${failure.target.presetName} (${normalizePath(failure.target.outputDir.relative)}):`,
    );
    for (const reason of failure.reasons) console.error(`    - ${reason}`);
  }
  console.error("  Run from the repository root:");
  console.error(`    ${rebuildCommandForTargets(targets)}`);
  process.exitCode = 1;
}

export function parseVerificationTargets(args) {
  assertKnownArgs(args, {
    valueArgs: ["--preset", "--out-dir-rel", "--surfaces"],
    booleanArgs: ["--all-surfaces", "--help", "-h"],
  });
  const allSurfaces = args.includes("--all-surfaces");
  const selectedSurfaces = parseArgValue(args, "--surfaces");
  const manualPreset = parseArgValue(args, "--preset");
  const manualOutput = parseArgValue(args, "--out-dir-rel");
  const modeCount = Number(allSurfaces) + Number(selectedSurfaces !== null);
  if (modeCount > 1) {
    throw new Error("--all-surfaces and --surfaces are mutually exclusive.");
  }
  if ((allSurfaces || selectedSurfaces !== null) && (manualPreset || manualOutput)) {
    throw new Error(
      "Surface selection cannot be combined with --preset or --out-dir-rel.",
    );
  }

  if (allSurfaces) return descriptorTargets(["root", ...publicSurfaces.keys()]);
  if (selectedSurfaces !== null) {
    const names = selectedSurfaces.split(",").map((name) => name.trim());
    if (names.length === 0 || names.some((name) => name.length === 0)) {
      throw new Error("--surfaces requires a comma-separated list of surface names.");
    }
    if (new Set(names).size !== names.length) {
      throw new Error("--surfaces must not contain duplicate names.");
    }
    return descriptorTargets(names);
  }

  return [
    {
      presetName:
        manualPreset ?? process.env.MERMAN_WEB_PRESET ?? defaultWebPresetName,
      outputDir: resolvePackageSubdir(
        webPackageRoot,
        manualOutput ?? "pkg",
        "--out-dir-rel",
      ),
    },
  ];
}

export function rebuildCommandForTargets(targets) {
  if (targets.length !== 1) {
    return "npm --prefix platforms/web run build";
  }
  const [{ presetName, outputDir }] = targets;
  const relative = normalizePath(outputDir.relative);
  if (presetName === defaultWebPresetName && relative === "pkg") {
    return "npm --prefix platforms/web run build:wasm";
  }
  return [
    "npm --prefix platforms/web run build:wasm --",
    `--preset ${presetName}`,
    `--out-dir-rel ${relative}`,
  ].join(" ");
}

function descriptorTargets(names) {
  return names.map((name) => {
    if (name === "root") {
      return {
        presetName: defaultWebPresetName,
        outputDir: resolvePackageSubdir(webPackageRoot, "pkg", "root surface"),
      };
    }
    const surface = publicSurfaces.get(name);
    if (!surface) {
      throw new Error(
        `Unknown Web surface '${name}'; expected root or one of: ${[...publicSurfaces.keys()].join(", ")}.`,
      );
    }
    return {
      presetName: surface.preset,
      outputDir: resolvePackageSubdir(
        webPackageRoot,
        surface.pkg_dir_rel,
        `surface ${name}`,
      ),
    };
  });
}

function verifyTarget(target, { getMetadata, getToolVersions }) {
  const preset = presets.get(target.presetName);
  if (!preset) {
    return {
      ok: false,
      reasons: [`Unknown @mermanjs/web WASM preset: ${target.presetName}`],
    };
  }
  const manifestPath = path.join(
    target.outputDir.absolute,
    WASM_INPUT_MANIFEST_NAME,
  );
  let manifest = null;
  if (existsSync(manifestPath)) {
    try {
      manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
    } catch (error) {
      return {
        ok: false,
        reasons: [
          `WASM input manifest is corrupt: ${error instanceof Error ? error.message : String(error)}`,
        ],
      };
    }
  }

  try {
    const result = verifyWasmInputManifest({
      allowedArtifactDirectories:
        normalizePath(target.outputDir.relative) === "pkg"
          ? publicSurfaceDirectoryNames()
          : [],
      manifest,
      metadata: manifest ? getMetadata(preset) : null,
      outputRoot: target.outputDir.absolute,
      preset,
      repoRoot: repositoryRoot,
      toolVersions: getToolVersions(),
    });
    return {
      ...result,
      digest: result.ok ? manifest.input_digest : null,
    };
  } catch (error) {
    return {
      ok: false,
      reasons: [error instanceof Error ? error.message : String(error)],
    };
  }
}

function printUsage() {
  console.log(
    "usage: node scripts/verify-wasm-inputs.mjs [--preset <name>] [--out-dir-rel <dir>] [--surfaces <root,editor,...> | --all-surfaces]",
  );
}

function normalizePath(value) {
  return value.split(path.sep).join("/");
}
