import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  parseVerificationTargets,
  rebuildCommandForTargets,
} from "./wasm-build/verify-cli.mjs";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

describe("WASM freshness CLI", () => {
  it("selects the exact pair of WASM surfaces consumed by Playground", () => {
    const targets = parseVerificationTargets(["--surfaces", "root,editor"]);
    assert.deepEqual(
      targets.map((target) => [
        target.presetName,
        normalizePath(target.outputDir.relative),
      ]),
      [
        ["browser-full", "pkg"],
        ["browser-editor", "pkg/editor"],
      ],
    );
    assert.equal(
      rebuildCommandForTargets(targets),
      "npm --prefix platforms/web run build",
    );
  });

  it("derives the all-surface set from the public descriptor", () => {
    const targets = parseVerificationTargets(["--all-surfaces"]);
    assert.deepEqual(
      targets.map((target) => normalizePath(target.outputDir.relative)),
      [
        "pkg",
        "pkg/core",
        "pkg/render",
        "pkg/render-only",
        "pkg/ascii",
        "pkg/editor",
        "pkg/full",
      ],
    );
  });

  it("rejects ambiguous, duplicate, and unknown surface selections", () => {
    assert.throws(
      () => parseVerificationTargets(["--all-surfaces", "--surfaces", "root"]),
      /mutually exclusive/,
    );
    assert.throws(
      () => parseVerificationTargets(["--surfaces", "root,root"]),
      /duplicate/,
    );
    assert.throws(
      () => parseVerificationTargets(["--surfaces", "root,missing"]),
      /Unknown Web surface/,
    );
  });

  it("prints a rebuild command whose package entrypoint runs from repository root", () => {
    const missingOutput = `pkg/.freshness-cli-test-${process.pid}`;
    const checked = spawnSync(
      process.execPath,
      [
        "platforms/web/scripts/verify-wasm-inputs.mjs",
        "--preset",
        "browser-core",
        "--out-dir-rel",
        missingOutput,
      ],
      {
        cwd: repositoryRoot,
        encoding: "utf8",
      },
    );
    assert.equal(checked.status, 1, checked.stdout + checked.stderr);
    const command = checked.stderr
      .split(/\r?\n/)
      .map((line) => line.trim())
      .find((line) => line.startsWith("npm --prefix platforms/web "));
    assert.ok(command, checked.stderr);

    const entrypoint = spawnSync(`${command} --help`, {
      cwd: repositoryRoot,
      encoding: "utf8",
      shell: true,
    });
    assert.equal(entrypoint.status, 0, entrypoint.stdout + entrypoint.stderr);
    assert.match(entrypoint.stdout, /usage: node scripts\/build-wasm\.mjs/);
  });
});

function normalizePath(value) {
  return value.split(path.sep).join("/");
}
