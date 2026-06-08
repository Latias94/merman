import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const generatedGitignore = path.join(root, "pkg", ".gitignore");
const generatedPackageJson = path.join(root, "pkg", "package.json");

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
