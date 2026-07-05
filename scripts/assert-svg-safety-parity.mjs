import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const helperDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(helperDir, "..");

export function assertSvgSafetySourcesInParity(repoRoot = defaultRepoRoot) {
  const webSource = fs.readFileSync(
    path.join(repoRoot, "platforms", "web", "src", "svg-safety.ts"),
    "utf8",
  );
  const vscodeSource = fs.readFileSync(
    path.join(repoRoot, "tools", "vscode-extension", "src", "preview-svg-safety.ts"),
    "utf8",
  );

  assert.equal(
    canonicalizeSvgSafetySource(webSource),
    canonicalizeSvgSafetySource(vscodeSource),
    "Web and VS Code SVG safety scanners must stay in policy parity.",
  );
}

export function canonicalizeSvgSafetySource(source) {
  return source
    .replace(/\r\n/g, "\n")
    .replaceAll("assertSafeSvgForDom", "assertSafeSvg")
    .replaceAll("assertSafePreviewSvg", "assertSafeSvg")
    .replaceAll("Merman rendered", "SvgSafetyScanner rejected")
    .replaceAll("Preview renderer returned", "SvgSafetyScanner rejected")
    .replace(/\s+/g, "")
    .replaceAll('",);', '");');
}
