import { mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import { downloadAndUnzipVSCode, runTests } from "@vscode/test-electron";

import { vscodeTestVersion } from "./vscode-test-version.mjs";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const extensionDevelopmentPath =
  process.env.MERMAN_VSCODE_EXTENSION_DEVELOPMENT_PATH ?? packageRoot;
const downloadAttempts = 3;

async function downloadVsCode() {
  for (let attempt = 1; ; attempt += 1) {
    try {
      return await downloadAndUnzipVSCode({
        version: vscodeTestVersion,
        timeout: 30_000,
      });
    } catch (error) {
      if (attempt >= downloadAttempts) {
        throw error;
      }
      const retryDelayMs = attempt * 1_000;
      console.warn(
        `VS Code download attempt ${attempt}/${downloadAttempts} failed; retrying in ${retryDelayMs}ms`,
        error
      );
      await delay(retryDelayMs);
    }
  }
}

const vscodeExecutablePath = await downloadVsCode();

for (const fixtureName of ["extension-host", "extension-host-lsp-failure"]) {
  const profileRoot = await mkdtemp(path.join(os.tmpdir(), "merman-vscode-"));
  try {
    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath: path.join(packageRoot, "dist", "extension-host-smoke.js"),
      vscodeExecutablePath,
      launchArgs: [
        path.join(packageRoot, "test-fixtures", fixtureName),
        `--user-data-dir=${path.join(profileRoot, "user-data")}`,
        `--extensions-dir=${path.join(profileRoot, "extensions")}`,
      ],
    });
  } finally {
    await rm(profileRoot, {
      recursive: true,
      force: true,
      maxRetries: 5,
      retryDelay: 100,
    });
  }
}
