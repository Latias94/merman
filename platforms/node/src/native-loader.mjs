import { createRequire } from "node:module";

import {
  MermanInvalidTransportError,
  MermanMissingPlatformPackageError,
  MermanNativeLoadError,
  MermanUnsupportedTargetError,
  parseRuntimeCatalogJsonText,
} from "./errors.mjs";

const TARGET_PACKAGES = Object.freeze({
  "darwin-arm64": "@mermanjs/node-darwin-arm64",
  "darwin-x64": "@mermanjs/node-darwin-x64",
  "linux-x64-gnu": "@mermanjs/node-linux-x64-gnu",
  "linux-x64-musl": "@mermanjs/node-linux-x64-musl",
  "win32-x64-msvc": "@mermanjs/node-win32-x64-msvc",
});

const requireFromPackage = createRequire(import.meta.url);
const loaderPackageManifest = requireFromPackage("../package.json");
const loaderPackageVersion = loaderPackageManifest?.version;

if (typeof loaderPackageVersion !== "string" || loaderPackageVersion.length === 0) {
  throw new Error("The Merman Node package manifest must declare a non-empty version.");
}

export function nodeLoaderPackageVersion() {
  return loaderPackageVersion;
}

export function assertRuntimePackageVersion(value) {
  const catalog = parseRuntimeCatalogJsonText(value);
  if (catalog?.package_version !== loaderPackageVersion) {
    throw new MermanInvalidTransportError(
      `The Merman runtime package version ${JSON.stringify(catalog?.package_version)} does not match the loader package version ${JSON.stringify(loaderPackageVersion)}.`,
    );
  }
  return value;
}

export function nativePackageName(target) {
  const packageName = TARGET_PACKAGES[target];
  if (!packageName) {
    throw new MermanUnsupportedTargetError({
      platform: target,
      arch: "unknown",
      reason: `Unknown Merman Node target id: ${target}.`,
    });
  }
  return packageName;
}

export function resolveNodeTarget({
  platform = process.platform,
  arch = process.arch,
  report = platform === "linux" ? currentProcessReport() : null,
} = {}) {
  if (platform === "darwin" && (arch === "arm64" || arch === "x64")) {
    return `darwin-${arch}`;
  }
  if (platform === "win32" && arch === "x64") return "win32-x64-msvc";
  if (platform === "linux" && arch === "x64") {
    const libc = linuxLibc(report);
    return `linux-x64-${libc}`;
  }
  throw new MermanUnsupportedTargetError({ platform, arch });
}

export function loadNativeBinding({
  platform = process.platform,
  arch = process.arch,
  report = platform === "linux" ? currentProcessReport() : null,
  loadPackage = requireFromPackage,
} = {}) {
  const target = resolveNodeTarget({ platform, arch, report });
  const packageName = nativePackageName(target);
  try {
    return loadPackage(packageName);
  } catch (cause) {
    if (isMissingModule(cause)) {
      throw new MermanMissingPlatformPackageError({ packageName, target, cause });
    }
    if (isNativeLoadFailure(cause)) {
      throw new MermanNativeLoadError({ packageName, target, cause });
    }
    throw cause;
  }
}

function linuxLibc(report) {
  if (report?.header?.glibcVersionRuntime) return "gnu";
  if (
    Array.isArray(report?.sharedObjects) &&
    report.sharedObjects.some((item) => /(?:^|[/\\])ld-musl-[^/\\]+\.so(?:\.1)?$/.test(item))
  ) {
    return "musl";
  }
  throw new MermanUnsupportedTargetError({
    platform: "linux",
    arch: "x64",
    reason:
      "Cannot determine Linux libc from process.report; refusing to guess a platform package.",
  });
}

function currentProcessReport() {
  return typeof process.report?.getReport === "function" ? process.report.getReport() : null;
}

function isMissingModule(error) {
  return Boolean(
    error &&
      typeof error === "object" &&
      (error.code === "MODULE_NOT_FOUND" || error.code === "ERR_MODULE_NOT_FOUND"),
  );
}

function isNativeLoadFailure(error) {
  if (!error || typeof error !== "object") return false;
  if (error.code === "ERR_DLOPEN_FAILED") return true;
  return typeof error.message === "string" &&
    /(?:GLIBC(?:XX)?_|CXXABI_|invalid ELF|wrong ELF|shared object file|not a valid Win32|mach-?o)/i.test(
      error.message,
    );
}
