import { existsSync, unlinkSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const generatedGitignore = path.join(root, "pkg", ".gitignore");

if (existsSync(generatedGitignore)) {
  unlinkSync(generatedGitignore);
}
