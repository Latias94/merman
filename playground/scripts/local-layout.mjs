import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";

import { WASM_INPUT_MANIFEST_NAME } from "../../platforms/web/scripts/wasm-build/input-manifest.mjs";
import { parseFullWasmTarget } from "../../platforms/web/scripts/wasm-build/targets.mjs";

const playgroundRoot = path.resolve(import.meta.dirname, "..");
const webRoot = path.resolve(playgroundRoot, "../platforms/web");
const webScripts = path.join(webRoot, "scripts");
const [action, ...args] = process.argv.slice(2);
const usage = "usage: node scripts/local-layout.mjs <assemble|prepare|dev|build> --local-layout <grid|layered>";

let target;
try {
  if (!["assemble", "prepare", "dev", "build"].includes(action)) {
    throw new Error("Expected a local Playground action.");
  }
  target = parseFullWasmTarget(args);
  if (target.localArgs[0] !== "--local-layout") {
    throw new Error("Select Grid or Layered with --local-layout.");
  }
  if (action === "build" && !process.env.npm_execpath) {
    throw new Error("Use npm run build:layout to build the local Playground.");
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  console.error(usage);
  process.exit(2);
}

try {
  const { localArgs } = target;
  if (action === "assemble") {
    const assembler = path.join(webScripts, "build-surface-packages.mjs");
    runNode(assembler, ["--entries"], webRoot);
    runNode(path.join(webScripts, "check-contracts.mjs"), localArgs, webRoot);
    runNode(path.join(webScripts, "clean-dist.mjs"), [], webRoot);
    runNode(path.join(webRoot, "node_modules/typescript/bin/tsc"), ["-p", "tsconfig.build.json"], webRoot);
    runNode(assembler, ["--assemble", "--package", "full", ...localArgs], webRoot);
  } else {
    runNode(path.join(webScripts, "verify-wasm-inputs.mjs"), ["--package", "full", "--assembled", ...localArgs], webRoot);
    const expectedWasmSha256 = action === "build" ? selectedWasmSha256(target) : null;
    runNode("scripts/build-opaque-realm.mjs");
    runNode("scripts/verify-opaque-realm.mjs");
    if (action === "dev") {
      runNode("node_modules/vite/bin/vite.js");
    } else if (action === "build") {
      runNode(process.env.npm_execpath, ["run", "build:prepared"], playgroundRoot, {
        ...process.env,
        MERMAN_PLAYGROUND_EXPECTED_WASM_SHA256: expectedWasmSha256,
      });
    }
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

function runNode(script, args = [], cwd = playgroundRoot, env = process.env) {
  const result = spawnSync(process.execPath, [script, ...args], { cwd, env, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function selectedWasmSha256({ outputDir }) {
  const input = JSON.parse(readFileSync(path.join(outputDir.absolute, WASM_INPUT_MANIFEST_NAME), "utf8"));
  const sha256 = input.artifacts.find((file) => file.path === "merman_wasm_bg.wasm")?.sha256;
  if (typeof sha256 !== "string" || !/^[a-f0-9]{64}$/u.test(sha256)) {
    throw new Error("The selected Web WASM input manifest is missing its binary SHA-256.");
  }
  return sha256;
}
