import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";

const repositoryRoot = path.resolve(import.meta.dirname, "../../..");
const webRoot = path.join(repositoryRoot, "platforms/web");
const packageRoot = path.join(webRoot, "packages/full");

export async function fullWebPackageProvenance(): Promise<{
  artifact_profile: string;
  runtime_capability_ids: readonly string[];
  outputs: readonly string[];
  artifact_files: unknown;
}> {
  return JSON.parse(await readFile(path.join(packageRoot, "artifacts/provenance.json"), "utf8"));
}

export async function verifyAssembledFullWebPackage(label: string): Promise<void> {
  const provenance = await fullWebPackageProvenance();
  const { fullWasmTargetForProvenance } = await import(
    pathToFileURL(path.join(webRoot, "scripts/wasm-build/targets.mjs")).href
  );
  const target = fullWasmTargetForProvenance(provenance);
  const args = ["platforms/web/scripts/verify-wasm-inputs.mjs", "--package", "full", "--assembled", ...target.localArgs];
  await promisify(execFile)(process.execPath, args, { cwd: repositoryRoot });
  const { assertArtifactFileEvidence } = await import(
    pathToFileURL(path.join(webRoot, "scripts/prepack-check.mjs")).href
  );
  assertArtifactFileEvidence({
    packageWasmRoot: path.join(packageRoot, "artifacts/wasm"),
    sourceWasmRoot: target.outputDir.absolute,
    packageDistRoot: path.join(packageRoot, "dist"),
    sourceDistRoot: path.join(webRoot, "dist"),
    packageId: "full",
    artifactFiles: provenance.artifact_files,
    label,
  });
}
