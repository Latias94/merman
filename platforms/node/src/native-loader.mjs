import { createRequire } from "node:module";

import {
  MermanInvalidTransportError,
  MermanMissingPlatformPackageError,
  MermanNativeLoadError,
  MermanUnsupportedTargetError,
  decodeWireCreationError,
  parseRuntimeCatalogJsonText,
  validateTransportIdentityExport,
} from "./errors.mjs";
import { wrapCandidateEngine } from "./candidates/wrap-engine.mjs";

const TARGET_PACKAGES = Object.freeze({
  "darwin-arm64": "@mermanjs/node-darwin-arm64",
  "darwin-x64": "@mermanjs/node-darwin-x64",
  "linux-arm64-gnu": "@mermanjs/node-linux-arm64-gnu",
  "linux-arm64-musl": "@mermanjs/node-linux-arm64-musl",
  "linux-x64-gnu": "@mermanjs/node-linux-x64-gnu",
  "linux-x64-musl": "@mermanjs/node-linux-x64-musl",
  "win32-x64-msvc": "@mermanjs/node-win32-x64-msvc",
});

const requireFromPackage = createRequire(import.meta.url);
// Only installed native addons can use the direct SVG input and result contract.
const directSvgTransports = new WeakSet();
const versionCheckedRuntimeCatalogs = new WeakMap();
const loaderPackageManifest = requireFromPackage("../package.json");
const loaderPackageVersion = loaderPackageManifest?.version;

if (typeof loaderPackageVersion !== "string" || loaderPackageVersion.length === 0) {
  throw new Error("The Merman Node package manifest must declare a non-empty version.");
}

export function nodeLoaderPackageVersion() {
  return loaderPackageVersion;
}

export function nodeLoaderUsesLocalShoal(packageName = "@mermanjs/node") {
  return loaderPackageManifest.name === packageName &&
    loaderPackageManifest.private === true && loaderPackageManifest.mermanLocalShoal === true;
}

export function cacheRuntimeCatalog(transport) {
  const text = transport.runtimeCatalogJson();
  const catalog = parseRuntimeCatalogJsonText(text);
  if (catalog?.package_version !== loaderPackageVersion) {
    throw new MermanInvalidTransportError(
      `The Merman runtime package version ${JSON.stringify(catalog?.package_version)} does not match the loader package version ${JSON.stringify(loaderPackageVersion)}.`,
    );
  }
  const wrapped = {
    ...transport,
    runtimeCatalogJson: () => text,
    dispose: () => {
      versionCheckedRuntimeCatalogs.delete(wrapped);
      return transport.dispose();
    },
  };
  versionCheckedRuntimeCatalogs.set(wrapped, { text, catalog });
  return wrapped;
}

export function takeVersionCheckedRuntimeCatalog(transport, text) {
  const checked = versionCheckedRuntimeCatalogs.get(transport);
  if (checked === undefined) return undefined;
  versionCheckedRuntimeCatalogs.delete(transport);
  // A custom loader can replace the catalog method on a wrapped transport.
  return checked.text === text ? checked.catalog : undefined;
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
  if (platform === "linux" && (arch === "arm64" || arch === "x64")) {
    const libc = linuxLibc(report, arch);
    return `linux-${arch}-${libc}`;
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

export async function loadInstalledNativeTransport(optionsJson, iconPacksJson) {
  let nativeAddon = false;
  const binding = loadNativeBinding({
    loadPackage(packageName) {
      const entrypoint = requireFromPackage.resolve(packageName);
      // The platform manifest loads the addon directly. JavaScript shims stay on the wire path.
      nativeAddon = entrypoint.endsWith(".node");
      return requireFromPackage(entrypoint);
    },
  });
  const transport = await wrapNativeBinding(binding, optionsJson, iconPacksJson, {
    directSvgAddon: nativeAddon,
    localShoal: nodeLoaderUsesLocalShoal(),
  });
  if (nativeAddon) {
    directSvgTransports.add(transport);
  }
  return transport;
}

export function supportsDirectNativeSvg(transport) {
  return directSvgTransports.has(transport);
}

export async function wrapNativeBinding(
  binding,
  optionsJson,
  iconPacksJson,
  { directSvgAddon = false, localShoal = false } = {},
) {
  const label = "The target-specific Merman addon";
  const NativeEngine = binding?.NativeEngine ?? binding?.default?.NativeEngine;
  if (typeof NativeEngine !== "function") {
    throw new MermanInvalidTransportError(
      "The target-specific Merman package does not export NativeEngine.",
    );
  }
  validateTransportIdentityExport(binding?.transportIdentityJson, {
    expectedPackageVersion: nodeLoaderPackageVersion(),
    expectedTransport: "napi",
    label: "The target-specific Merman package",
    localShoal,
  });
  let transport;
  try {
    transport = wrapCandidateEngine(
      iconPacksJson === undefined
        ? new NativeEngine(optionsJson)
        : new NativeEngine(optionsJson, iconPacksJson),
      label,
      { forwardsAbortSignal: true, directSvgAddon },
    );
  } catch (cause) {
    throw decodeWireCreationError(cause, label);
  }
  try {
    const wrapped = cacheRuntimeCatalog(transport);
    return typeof wrapped.executeSvg === "function" ? Object.freeze(wrapped) : wrapped;
  } catch (cause) {
    try {
      await transport.dispose();
    } catch {
      // Preserve the package-version failure that made the transport unusable.
    }
    throw cause;
  }
}

function linuxLibc(report, arch) {
  if (report?.header?.glibcVersionRuntime) return "gnu";
  if (
    Array.isArray(report?.sharedObjects) &&
    report.sharedObjects.some((item) => /(?:^|[/\\])ld-musl-[^/\\]+\.so(?:\.1)?$/.test(item))
  ) {
    return "musl";
  }
  throw new MermanUnsupportedTargetError({
    platform: "linux",
    arch,
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
