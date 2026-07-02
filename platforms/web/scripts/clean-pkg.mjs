import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const pkgDirRel = parsePkgDirRel(process.argv.slice(2)) ?? "pkg";
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

function parsePkgDirRel(args) {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--pkg-dir-rel") {
      return args[index + 1];
    }
    if (arg.startsWith("--pkg-dir-rel=")) {
      return arg.slice("--pkg-dir-rel=".length);
    }
  }
  return null;
}
