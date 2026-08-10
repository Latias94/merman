import { readFile } from "node:fs/promises";
import path from "node:path";

const LIFECYCLE_SCRIPTS = ["preinstall", "install", "postinstall"];

export async function inspectPackageManifests(nodeRoot, descriptor) {
  if (descriptor?.schema_version !== 1 || descriptor?.admission_status !== "public-alpha") {
    throw new Error("Node package surface descriptor must define the schema-1 public alpha group.");
  }
  const root = await inspectManifest(nodeRoot, descriptor.root, "loader");
  const targets = [];
  for (const targetDescriptor of descriptor.targets) {
    targets.push({
      target: targetDescriptor.target,
      ...(await inspectManifest(nodeRoot, targetDescriptor, "platform")),
    });
  }
  const versions = new Set([root.manifest.version, ...targets.map((item) => item.manifest.version)]);
  if (versions.size !== 1 || !versions.has(descriptor.version)) {
    throw new Error("Node candidate package versions must be exact and lockstep.");
  }
  const engineRanges = new Set([
    root.manifest.engines?.node,
    ...targets.map((item) => item.manifest.engines?.node),
  ]);
  if (
    typeof descriptor.node_engine !== "string" ||
    descriptor.node_engine.length === 0 ||
    engineRanges.size !== 1 ||
    !engineRanges.has(descriptor.node_engine)
  ) {
    throw new Error("Node candidate package engine ranges must be explicit and lockstep.");
  }
  for (const item of targets) {
    if (root.manifest.optionalDependencies?.[item.manifest.name] !== descriptor.version) {
      throw new Error(`${item.manifest.name} must be an exact-version optional dependency.`);
    }
    if (
      item.manifest.os?.length !== 1 ||
      item.manifest.os[0] !== item.descriptor.os ||
      item.manifest.cpu?.length !== 1 ||
      item.manifest.cpu[0] !== item.descriptor.cpu ||
      JSON.stringify(item.manifest.libc ?? []) !==
        JSON.stringify(item.descriptor.libc ? [item.descriptor.libc] : [])
    ) {
      throw new Error(`${item.manifest.name} platform constraints must match its descriptor.`);
    }
  }
  const optionalNames = Object.keys(root.manifest.optionalDependencies ?? {}).sort();
  const targetNames = targets.map((item) => item.manifest.name).sort();
  if (JSON.stringify(optionalNames) !== JSON.stringify(targetNames)) {
    throw new Error("Node loader optional dependencies must exactly match the target package set.");
  }
  return { root, targets };
}

export function verifyPackedFileOwnership({ packageName, role, files }) {
  const paths = files.map((item) => item.path);
  const nodeFiles = paths.filter((item) => item.endsWith(".node"));
  const wasmFiles = paths.filter((item) => item.endsWith(".wasm"));
  if (role === "loader" && nodeFiles.length !== 0) {
    throw new Error(`${packageName} loader package must not contain native binaries.`);
  }
  if (role === "loader" && wasmFiles.length !== 0) {
    throw new Error(`${packageName} loader package must not contain WASM binaries.`);
  }
  if (role === "platform" && nodeFiles.length !== 1) {
    throw new Error(`${packageName} platform package must contain exactly one .node binary.`);
  }
  if (role === "platform" && wasmFiles.length !== 0) {
    throw new Error(`${packageName} platform package must not contain WASM binaries.`);
  }
}

async function inspectManifest(nodeRoot, descriptor, role) {
  const manifestPath = path.join(nodeRoot, descriptor.directory, "package.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  if (manifest.name !== descriptor.name || manifest.private === true) {
    throw new Error(`${descriptor.name} must be a public npm package.`);
  }
  if (
    manifest.publishConfig?.access !== "public" ||
    manifest.repository?.url !== "git+https://github.com/Latias94/merman.git"
  ) {
    throw new Error(`${descriptor.name} must retain public npm and repository metadata.`);
  }
  if (manifest.scripts && LIFECYCLE_SCRIPTS.some((name) => name in manifest.scripts)) {
    throw new Error(`${descriptor.name} must not download artifacts from npm lifecycle scripts.`);
  }
  const declaredFiles = Array.isArray(manifest.files) ? manifest.files : [];
  const nodeFiles = declaredFiles.filter((item) => item.endsWith(".node"));
  const wasmFiles = declaredFiles.filter((item) => item.endsWith(".wasm"));
  if (role === "platform" && (nodeFiles.length !== 1 || nodeFiles[0] !== descriptor.node_artifact)) {
    throw new Error(`${descriptor.name} must own exactly ${descriptor.node_artifact}.`);
  }
  const dependencyNames = Object.keys({
    ...manifest.dependencies,
    ...manifest.optionalDependencies,
    ...manifest.peerDependencies,
  });
  return {
    descriptor,
    manifest,
    nodeArtifact: nodeFiles[0] ?? null,
    nodeFiles,
    wasmFiles,
    hasLifecycleDownload: Boolean(
      manifest.scripts && LIFECYCLE_SCRIPTS.some((name) => name in manifest.scripts),
    ),
    hasBrowserFallback: dependencyNames.some((name) => name.startsWith("@mermanjs/web")),
  };
}
