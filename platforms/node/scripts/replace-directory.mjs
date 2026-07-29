import { existsSync, mkdirSync, renameSync, rmSync } from "node:fs";
import path from "node:path";

export function replaceDirectory(stage, output) {
  mkdirSync(path.dirname(output), { recursive: true });
  const backup = `${output}.backup`;
  rmSync(backup, { recursive: true, force: true });
  if (existsSync(output)) renameSync(output, backup);
  try {
    renameSync(stage, output);
    rmSync(backup, { recursive: true, force: true });
  } catch (error) {
    rmSync(output, { recursive: true, force: true });
    if (existsSync(backup)) renameSync(backup, output);
    throw error;
  }
}
