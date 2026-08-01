import { readFileSync } from "node:fs";

const manifest = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url), "utf8"),
);

export const vscodeTestVersion = minimumSupportedVscodeVersion(
  manifest.engines?.vscode,
);

export function minimumSupportedVscodeVersion(versionRange) {
  const match = /^\^(\d+\.\d+\.\d+)$/.exec(versionRange ?? "");
  if (!match) {
    throw new Error(
      `Expected package.json engines.vscode to be an exact caret range, got ${JSON.stringify(versionRange)}`,
    );
  }
  return match[1];
}
