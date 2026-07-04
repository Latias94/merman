#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(packageDir, "..", "..");
const vsceTargets = new Set([
  "win32-x64",
  "win32-arm64",
  "linux-x64",
  "linux-arm64",
  "linux-armhf",
  "darwin-x64",
  "darwin-arm64",
  "alpine-x64",
  "alpine-arm64",
  "web",
]);

if (isDirectRun()) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
  }
}

export function main(argv, options = {}) {
  const packageRoot = options.packageDir ?? packageDir;
  const workspaceRoot = options.repoRoot ?? repoRoot;
  const manifestPath = path.join(packageRoot, "package.json");
  const vsceCliPath = path.join(packageRoot, "node_modules", "@vscode", "vsce", "vsce");
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  const releaseVersion =
    options.releaseVersion ?? process.env.MERMAN_RELEASE_VERSION ?? readWorkspacePackageVersion(workspaceRoot);
  const { args, message } = buildVscePackageArgs({
    manifestVersion: manifest.version,
    releaseVersion,
    userArgs: argv,
    env: process.env,
  });

  if (message) {
    console.log(message);
  }

  const result = spawnSync(process.execPath, [vsceCliPath, ...args], {
    cwd: packageRoot,
    stdio: "inherit",
  });

  if (result.error) {
    fail(`Failed to run local vsce CLI: ${result.error.message}`);
  }
  process.exit(result.status ?? 1);
}

function readWorkspacePackageVersion(root) {
  const cargoToml = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const match = cargoToml.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    fail("Could not find [workspace.package] version in Cargo.toml.");
  }
  return match[1];
}

export function buildVscePackageArgs({ manifestVersion, releaseVersion, userArgs, env = process.env }) {
  const parsedManifest = parseSourceVersion(manifestVersion, "package.json version");
  const parsedRelease = parseSourceVersion(releaseVersion, "release version");

  if (parsedManifest.raw !== parsedRelease.stableVersion) {
    throw new Error(
      `VS Code package.json version must be the stable VSIX manifest version for ${parsedRelease.raw}: expected ${parsedRelease.stableVersion}, got ${parsedManifest.raw}.`,
    );
  }

  const args = ["package"];
  let message = null;
  if (parsedRelease.preRelease !== null) {
    args.push("--pre-release");
    message =
      `Packaging prerelease source version ${parsedRelease.raw} as VSIX version ${parsedRelease.stableVersion} with --pre-release.`;
  }
  args.push(...normalizeNpmForwardedArgs(userArgs, env));
  return { args, message };
}

export function parseSourceVersion(raw, label) {
  if (typeof raw !== "string") {
    throw new Error(`${label} must be a string.`);
  }
  const match = raw.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) {
    throw new Error(`${label} is not valid SemVer: ${JSON.stringify(raw)}.`);
  }
  return {
    raw,
    stableVersion: `${match[1]}.${match[2]}.${match[3]}`,
    preRelease: match[4] ?? null,
  };
}

export function normalizeNpmForwardedArgs(args, env = process.env) {
  let normalized = [...args];
  for (const option of ["target", "out"]) {
    const value = env[`npm_config_${option}`];
    const flag = `--${option}`;
    if (!value || value === "true" || hasOption(normalized, flag)) {
      continue;
    }
    normalized = normalized.filter((arg) => arg !== value);
    normalized.push(flag, value);
  }
  normalized = normalizeBareForwardedArgs(normalized);
  return normalized;
}

export function normalizeBareForwardedArgs(args) {
  const normalized = [];
  for (const arg of args) {
    if (!arg.startsWith("-") && vsceTargets.has(arg) && !hasOption([...normalized, ...args], "--target")) {
      normalized.push("--target", arg);
    } else if (!arg.startsWith("-") && arg.endsWith(".vsix") && !hasOption([...normalized, ...args], "--out")) {
      normalized.push("--out", arg);
    } else {
      normalized.push(arg);
    }
  }
  return normalized;
}

export function hasOption(args, flag) {
  return args.some((arg) => arg === flag || arg.startsWith(`${flag}=`));
}

function isDirectRun() {
  return process.argv[1] !== undefined && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
