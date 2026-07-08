import path from "node:path";

import { assertKnownArgs, parseArgValue, resolvePackageSubdir } from "./arg-parse.mjs";

export function parseSmokeCli(inputArgs, packageRoot) {
  assertKnownArgs(inputArgs, {
    valueArgs: [
      "--entry",
      "--pkg-dir-rel",
      "--wasm-module-subpath",
      "--wasm-binary-rel",
      "--manifest-rel",
    ],
  });
  const selectedPkgDirRel = parseArgValue(inputArgs, "--pkg-dir-rel") ?? "pkg";
  const pkgDir = resolvePackageSubdir(packageRoot, selectedPkgDirRel, "--pkg-dir-rel");
  const pkgDirRel = normalizePath(pkgDir.relative);
  return {
    entrySubpath: parseArgValue(inputArgs, "--entry") ?? ".",
    pkgDirRel,
    wasmModuleSubpath:
      parseArgValue(inputArgs, "--wasm-module-subpath") ?? `./${pkgDirRel}/merman_wasm.js`,
    wasmBinaryRel:
      parseArgValue(inputArgs, "--wasm-binary-rel") ??
      normalizePath(path.join(pkgDirRel, "merman_wasm_bg.wasm")),
    manifestRel:
      parseArgValue(inputArgs, "--manifest-rel") ??
      normalizePath(path.join(pkgDirRel, "merman_wasm_preset.json")),
  };
}

export function smokeUsage() {
  return "usage: node scripts/smoke.mjs [--entry <subpath>] [--pkg-dir-rel <dir>] [--wasm-module-subpath <subpath>] [--wasm-binary-rel <path>] [--manifest-rel <path>]";
}

function normalizePath(value) {
  return value.split(path.sep).join("/");
}
