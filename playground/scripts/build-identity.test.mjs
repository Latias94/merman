import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { readBuildIdentity } from "./build-identity.mjs";

function checkout(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "merman-build-identity-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const git = (...args) => execFileSync("git", args, {
    cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"],
  }).trim();
  git("init", "--initial-branch=main");
  git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
    "-c", "commit.gpgsign=false", "commit", "--allow-empty", "-m", "test");
  return { root, commit: git("rev-parse", "HEAD") };
}

test("main, preview and tag builds identify the actual detached checkout", (t) => {
  const { root, commit } = checkout(t);
  execFileSync("git", ["switch", "--detach", commit], { cwd: root, stdio: "ignore" });
  for (const [ref, channel] of [
    ["refs/heads/main", "main"], ["refs/heads/master", "main"],
    ["refs/pull/164/merge", "preview"],
    ["refs/tags/v0.8.0", "tag"],
  ]) {
    assert.deepEqual(readBuildIdentity(root, {
      GITHUB_ACTIONS: "true", GITHUB_SHA: commit,
      GITHUB_REF: ref, GITHUB_REPOSITORY: "Latias94/merman",
    }), { channel, commit, ref, dirty: false, repository: "Latias94/merman" });
  }
  assert.throws(() => readBuildIdentity(root, {
    GITHUB_ACTIONS: "true", GITHUB_SHA: "0".repeat(40),
  }), /does not match/);
});

test("local modifications do not claim to be an exact deployed commit", (t) => {
  const { root, commit } = checkout(t);
  assert.equal(readBuildIdentity(root, {}).dirty, false);
  fs.writeFileSync(path.join(root, "new-source.ts"), "export {};\n");
  assert.deepEqual(readBuildIdentity(root, {}), {
    channel: "local", commit, ref: null, dirty: true, repository: null,
  });
});

test("source archives show local identity, while CI requires provenance", (t) => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "merman-build-archive-"));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  assert.deepEqual(readBuildIdentity(root, {}), {
    channel: "local", commit: null, ref: null, dirty: false, repository: null,
  });
  assert.throws(() => readBuildIdentity(root, { GITHUB_ACTIONS: "true" }), /readable Git/);
});
