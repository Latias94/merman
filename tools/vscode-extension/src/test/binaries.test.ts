import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { afterEach, describe, it } from "node:test";

import {
  binaryFileName,
  findPackagedBinary,
  platformKey,
  resolveMermanBinary,
} from "../binaries.js";

const tempDirs: string[] = [];

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

describe("Merman binary resolution", () => {
  it("uses a trusted configured executable path first", () => {
    const root = tempDir();
    const explicitRoot = tempDir();
    const explicit = touchExecutable(path.join(explicitRoot, "custom-lsp"));
    const packaged = touchExecutable(
      path.join(root, "extension", "bin", "linux-x64", "merman-lsp"),
    );

    const invocation = resolveMermanBinary({
      binaryName: "merman-lsp",
      packageName: "merman-lsp",
      extensionPath: path.join(root, "extension"),
      workspaceRoots: [root],
      explicitPath: explicit,
      directArgs: ["--stdio"],
      workspaceTrusted: false,
      platform: "linux",
      arch: "x64",
    });

    assert.equal(invocation.command, explicit);
    assert.deepEqual(invocation.args, ["--stdio"]);
    assert.equal(invocation.source, "explicit");
    assert.equal(findPackagedBinary({
      binaryName: "merman-lsp",
      extensionPath: path.join(root, "extension"),
      platform: "linux",
      arch: "x64",
    }), packaged);
  });

  it("uses packaged binaries without scanning workspace debug binaries", () => {
    const root = tempDir();
    const extensionPath = path.join(root, "extension");
    const packaged = touchExecutable(
      path.join(extensionPath, "bin", "darwin-arm64", "merman-cli"),
    );
    touchExecutable(path.join(root, "target", "debug", "merman-cli"));

    const invocation = resolveMermanBinary({
      binaryName: "merman-cli",
      packageName: "merman-cli",
      extensionPath,
      workspaceRoots: [root],
      directArgs: ["-q"],
      platform: "darwin",
      arch: "arm64",
    });

    assert.equal(invocation.command, packaged);
    assert.equal(invocation.source, "packaged");
  });

  it("does not fall back to workspace debug binaries by default", () => {
    const root = tempDir();
    touchExecutable(path.join(root, "target", "debug", "merman-lsp"));

    assert.throws(
      () =>
        resolveMermanBinary({
          binaryName: "merman-lsp",
          packageName: "merman-lsp",
          extensionPath: path.join(root, "extension"),
          workspaceRoots: [root],
          platform: "linux",
          arch: "x64",
        }),
      /Unable to find merman-lsp/,
    );
  });

  it("rejects configured workspace-local executables in untrusted workspaces", () => {
    const root = tempDir();
    const explicit = touchExecutable(path.join(root, "tools", "merman-lsp"));

    assert.throws(
      () =>
        resolveMermanBinary({
          binaryName: "merman-lsp",
          packageName: "merman-lsp",
          extensionPath: path.join(root, "extension"),
          workspaceRoots: [root],
          explicitPath: explicit,
          workspaceTrusted: false,
          platform: "linux",
          arch: "x64",
        }),
      /requires a trusted workspace/,
    );
  });

  it("allows configured workspace-local executables in trusted workspaces", () => {
    const root = tempDir();
    const explicit = touchExecutable(path.join(root, "tools", "merman-lsp"));

    const invocation = resolveMermanBinary({
      binaryName: "merman-lsp",
      packageName: "merman-lsp",
      extensionPath: path.join(root, "extension"),
      workspaceRoots: [root],
      explicitPath: explicit,
      workspaceTrusted: true,
      platform: "linux",
      arch: "x64",
    });

    assert.equal(invocation.command, explicit);
    assert.equal(invocation.source, "explicit");
  });

  it("uses Cargo only when explicitly enabled", () => {
    const root = tempDir();

    const invocation = resolveMermanBinary({
      binaryName: "merman-cli",
      packageName: "merman-cli",
      extensionPath: path.join(root, "extension"),
      workspaceRoots: [root],
      directArgs: ["-i", "-"],
      useCargoRun: true,
      cargoArgs: ["--release"],
      workspaceTrusted: true,
    });

    assert.equal(invocation.command, "cargo");
    assert.deepEqual(invocation.args, [
      "run",
      "-p",
      "merman-cli",
      "--release",
      "--",
      "-i",
      "-",
    ]);
    assert.equal(invocation.cwd, root);
    assert.equal(invocation.source, "cargo");
  });

  it("rejects Cargo fallback in untrusted workspaces", () => {
    const root = tempDir();

    assert.throws(
      () =>
        resolveMermanBinary({
          binaryName: "merman-cli",
          packageName: "merman-cli",
          extensionPath: path.join(root, "extension"),
          workspaceRoots: [root],
          useCargoRun: true,
          workspaceTrusted: false,
        }),
      /Cargo development fallback requires a trusted workspace/,
    );
  });

  it("throws a setup error when no runtime binary is available", () => {
    const root = tempDir();

    assert.throws(
      () =>
        resolveMermanBinary({
          binaryName: "merman-lsp",
          packageName: "merman-lsp",
          extensionPath: path.join(root, "extension"),
          workspaceRoots: [root],
          platform: "linux",
          arch: "x64",
        }),
      /Unable to find merman-lsp/,
    );
  });

  it("maps platform binary names and keys", () => {
    assert.equal(binaryFileName("merman-cli", "win32"), "merman-cli.exe");
    assert.equal(binaryFileName("merman-cli", "linux"), "merman-cli");
    assert.equal(platformKey("darwin", "arm64"), "darwin-arm64");
  });
});

function tempDir(): string {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "merman-vscode-test-"));
  tempDirs.push(dir);
  return dir;
}

function touchExecutable(filePath: string): string {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, "");
  fs.chmodSync(filePath, 0o755);
  return filePath;
}
