import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");

export const canonicalSvgSafetyPolicyPath = path.join(
  repoRoot,
  "platforms",
  "web",
  "src",
  "svg-safety-policy.ts",
);

export const generatedSvgSafetyPolicyPath = path.join(
  repoRoot,
  "tools",
  "vscode-extension",
  "src",
  "preview-svg-safety-policy.ts",
);

export async function generateSvgSafetyPolicy() {
  const canonicalPolicy = await fs.readFile(canonicalSvgSafetyPolicyPath, "utf8");
  await fs.mkdir(path.dirname(generatedSvgSafetyPolicyPath), { recursive: true });
  await fs.writeFile(generatedSvgSafetyPolicyPath, canonicalPolicy);
}

export async function assertGeneratedSvgSafetyPolicyCurrent() {
  const [canonicalPolicy, generatedPolicy] = await Promise.all([
    fs.readFile(canonicalSvgSafetyPolicyPath, "utf8"),
    fs.readFile(generatedSvgSafetyPolicyPath, "utf8"),
  ]);

  if (canonicalPolicy === generatedPolicy) {
    return;
  }

  throw new Error(
    [
      "Generated VS Code SVG safety policy is stale.",
      `Source: ${path.relative(repoRoot, canonicalSvgSafetyPolicyPath)}`,
      `Generated: ${path.relative(repoRoot, generatedSvgSafetyPolicyPath)}`,
      "Run: node scripts/generate-svg-safety-policy.mjs",
    ].join("\n"),
  );
}
