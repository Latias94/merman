import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";

import { assertKnownArgs, parseArgValue, resolvePackageSubdir } from "./arg-parse.mjs";
import { webPackageRoot } from "./paths.mjs";

export function cleanPackageOutput(packageOutputRoot) {
  const generatedGitignore = path.join(packageOutputRoot, ".gitignore");
  const generatedPackageJson = path.join(packageOutputRoot, "package.json");

  if (existsSync(generatedGitignore)) unlinkSync(generatedGitignore);
  if (!existsSync(generatedPackageJson)) return;

  const packageJson = JSON.parse(readFileSync(generatedPackageJson, "utf8"));
  if (packageJson.type !== "module") {
    packageJson.type = "module";
    writeFileSync(generatedPackageJson, `${JSON.stringify(packageJson, null, 2)}\n`);
  }
}
export function runCleanPackageCli(args = process.argv.slice(2)) {
  try {
    assertKnownArgs(args, { valueArgs: ["--pkg-dir-rel"] });
    const output = resolvePackageSubdir(
      webPackageRoot,
      parseArgValue(args, "--pkg-dir-rel") ?? "pkg",
      "--pkg-dir-rel",
    );
    cleanPackageOutput(output.absolute);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error("usage: node scripts/clean-pkg.mjs [--pkg-dir-rel <dir>]");
    process.exitCode = 2;
  }
}
