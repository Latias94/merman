import { spawnSync } from "node:child_process";

import { DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH } from "./contract.mjs";
import { sha256 } from "./measurement-shared.mjs";

export function repositoryRevision(repositoryRoot) {
  const commit = captureCommand("git", ["rev-parse", "HEAD"], repositoryRoot);
  const status = captureCommand(
    "git",
    ["status", "--porcelain=v1", "--untracked-files=all"],
    repositoryRoot,
  );
  return {
    commit,
    dirty: status.length > 0,
    statusSha256: sha256(Buffer.from(status)),
  };
}

export function assertSameRevision(before, after) {
  if (
    before.commit !== after.commit ||
    before.dirty !== after.dirty ||
    before.statusSha256 !== after.statusSha256
  ) {
    throw new Error(
      "Repository revision or worktree status changed during editor artifact measurement.",
    );
  }
}

export function consistentBrowserVersion(current, observed) {
  if (current !== null && current !== observed) {
    throw new Error(
      `Editor artifact measurement switched Chromium versions from ${current} to ${observed}.`,
    );
  }
  return observed;
}

export function runCommand(command, args, cwd, env = process.env) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: "utf8",
    env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      `${command} ${args.join(" ")} failed with ${result.status}.`,
    );
  }
}

function captureCommand(command, args, cwd) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(
      result.stderr.trim() || `${command} failed with ${result.status}.`,
    );
  }
  return result.stdout.trim();
}

export function parseOptions(args) {
  const parsed = {
    blocks: 4,
    headed: false,
    help: false,
    out: null,
    skipBuild: false,
  };
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--headed") parsed.headed = true;
    else if (arg === "--help" || arg === "-h") parsed.help = true;
    else if (arg === "--skip-build") parsed.skipBuild = true;
    else if (arg === "--blocks" || arg === "--out") {
      const value = args[index + 1];
      if (!value || value.startsWith("--"))
        throw new Error(`Missing value for ${arg}.`);
      index += 1;
      if (arg === "--blocks") {
        parsed.blocks = Number(value);
        if (
          !Number.isSafeInteger(parsed.blocks) ||
          parsed.blocks < 2 ||
          parsed.blocks % 2 !== 0
        ) {
          throw new Error(
            "--blocks must be an even integer of at least 2 for balanced AB/BA evidence.",
          );
        }
      } else parsed.out = value;
    } else throw new Error(`Unknown argument: ${arg}`);
  }
  return parsed;
}

export function printUsage() {
  console.log(
    [
      "usage: node scripts/editor-artifact-measurement/measure.mjs [options]",
      "",
      "  --blocks <count>  balanced AB/BA block count (default: 4, even and at least 2)",
      "  --headed          show Chromium during measurement",
      `  --out <path>      receipt path (default: ${DEFAULT_EDITOR_ARTIFACT_RECEIPT_PATH})`,
      "  --skip-build      reuse existing dedicated full/editor build directories",
    ].join("\n"),
  );
}

export function npmCommand() {
  return process.platform === "win32" ? "npm.cmd" : "npm";
}
