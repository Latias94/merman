#!/usr/bin/env node
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";

const extensionRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(extensionRoot, "../..");
const platformKey = `${process.platform}-${process.arch}`;
const executableSuffix = process.platform === "win32" ? ".exe" : "";
const targetProfile = process.env.MERMAN_VSCODE_TARGET_PROFILE ?? "release";
const targetDir =
  process.env.MERMAN_VSCODE_TARGET_DIR ??
  path.join(repoRoot, "target", targetProfile);
const outDir =
  process.env.MERMAN_VSCODE_BIN_DIR ??
  path.join(extensionRoot, "bin", platformKey);
const binaries = ["merman-lsp", "merman-cli"];

fs.mkdirSync(outDir, { recursive: true });

const missing = [];
for (const binary of binaries) {
  const fileName = `${binary}${executableSuffix}`;
  const source = path.join(targetDir, fileName);
  const destination = path.join(outDir, fileName);
  if (!fs.existsSync(source)) {
    missing.push(source);
    continue;
  }
  fs.copyFileSync(source, destination);
  if (os.platform() !== "win32") {
    fs.chmodSync(destination, 0o755);
  }
  console.log(`copied ${path.relative(extensionRoot, destination)}`);
}

if (missing.length > 0) {
  console.error("Missing Merman runtime binaries:");
  for (const filePath of missing) {
    console.error(`- ${filePath}`);
  }
  console.error("Build them first with the separate exact artifact recipes:");
  console.error(
    "- cargo build --release --locked --manifest-path crates/merman-lsp/Cargo.toml -p merman-lsp --bin merman-lsp --no-default-features --features stdio",
  );
  console.error(
    "- cargo build --release --locked --manifest-path crates/merman-cli/Cargo.toml -p merman-cli --bin merman-cli --no-default-features --features analysis,ascii,icons,jpeg,layout-cytoscape,layout-elk,markdown,math,network-icons,parallel-markdown,pdf,png,shell-completions,svg,system-clock,system-random,system-timezone,system-timing",
  );
  process.exit(1);
}
