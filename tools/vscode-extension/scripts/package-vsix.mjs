#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const packageDir = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(packageDir, "..", "..");
const manifestPath = path.join(packageDir, "package.json");
const vsceCliPath = path.join(packageDir, "node_modules", "@vscode", "vsce", "vsce");
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

const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const manifestVersion = parseSourceVersion(manifest.version, "package.json version");
const releaseVersion = parseSourceVersion(
  process.env.MERMAN_RELEASE_VERSION ?? readWorkspacePackageVersion(repoRoot),
  "release version",
);
const userArgs = normalizeNpmForwardedArgs(process.argv.slice(2));

if (manifestVersion.raw !== releaseVersion.stableVersion) {
  fail(
    `VS Code package.json version must be the stable VSIX manifest version for ${releaseVersion.raw}: expected ${releaseVersion.stableVersion}, got ${manifestVersion.raw}.`,
  );
}

const vsceArgs = ["package"];
if (releaseVersion.preRelease !== null) {
  vsceArgs.push("--pre-release");
  console.log(
    `Packaging prerelease source version ${releaseVersion.raw} as VSIX version ${releaseVersion.stableVersion} with --pre-release.`,
  );
}
vsceArgs.push(...userArgs);

const result = spawnSync(process.execPath, [vsceCliPath, ...vsceArgs], {
  cwd: packageDir,
  stdio: "inherit",
});

if (result.error) {
  console.error(`Failed to run local vsce CLI: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status ?? 1);

function readWorkspacePackageVersion(root) {
  const cargoToml = fs.readFileSync(path.join(root, "Cargo.toml"), "utf8");
  const match = cargoToml.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    fail("Could not find [workspace.package] version in Cargo.toml.");
  }
  return match[1];
}

function parseSourceVersion(raw, label) {
  if (typeof raw !== "string") {
    fail(`${label} must be a string.`);
  }
  const match = raw.match(/^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) {
    fail(`${label} is not valid SemVer: ${JSON.stringify(raw)}.`);
  }
  return {
    raw,
    stableVersion: `${match[1]}.${match[2]}.${match[3]}`,
    preRelease: match[4] ?? null,
  };
}

function normalizeNpmForwardedArgs(args) {
  let normalized = [...args];
  for (const option of ["target", "out"]) {
    const value = process.env[`npm_config_${option}`];
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

function normalizeBareForwardedArgs(args) {
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

function hasOption(args, flag) {
  return args.some((arg) => arg === flag || arg.startsWith(`${flag}=`));
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
