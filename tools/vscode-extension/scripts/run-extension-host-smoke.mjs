import path from "node:path";
import { fileURLToPath } from "node:url";
import { runTests } from "@vscode/test-electron";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

for (const fixtureName of ["extension-host", "extension-host-lsp-failure"]) {
  await runTests({
    extensionDevelopmentPath: packageRoot,
    extensionTestsPath: path.join(packageRoot, "dist", "extension-host-smoke.js"),
    launchArgs: [
      path.join(packageRoot, "test-fixtures", fixtureName),
    ],
  });
}
