import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { assertKnownArgs, parseArgValue } from "./arg-parse.mjs";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkgDirRel = parseCli(process.argv.slice(2));
const pkgRoot = path.join(root, pkgDirRel);
const generatedGitignore = path.join(pkgRoot, ".gitignore");
const generatedPackageJson = path.join(pkgRoot, "package.json");

if (existsSync(generatedGitignore)) {
  unlinkSync(generatedGitignore);
}

if (existsSync(generatedPackageJson)) {
  const packageJson = JSON.parse(readFileSync(generatedPackageJson, "utf8"));
  if (packageJson.type !== "module") {
    packageJson.type = "module";
    writeFileSync(generatedPackageJson, `${JSON.stringify(packageJson, null, 2)}\n`);
  }
}

function parseCli(args) {
  try {
    assertKnownArgs(args, { valueArgs: ["--pkg-dir-rel"] });
    return parseArgValue(args, "--pkg-dir-rel") ?? "pkg";
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    console.error("usage: node scripts/clean-pkg.mjs [--pkg-dir-rel <dir>]");
    process.exit(2);
  }
}
