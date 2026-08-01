import { readFile } from "node:fs/promises";
import path from "node:path";

const LIFECYCLE_SCRIPTS = ["preinstall", "install", "postinstall"];

export async function inspectPackageManifests(nodeRoot, descriptor) {
  if (descriptor?.schema_version !== 1 || descriptor?.admission_status !== "candidate") {
    throw new Error("Node package surface descriptor must remain a schema-1 private candidate.");
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
  for (const item of targets) {
    if (root.manifest.optionalDependencies?.[item.manifest.name] !== descriptor.version) {
      throw new Error(`${item.manifest.name} must be an exact-version optional dependency.`);
    }
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
  if (manifest.name !== descriptor.name || manifest.private !== true) {
    throw new Error(`${descriptor.name} must remain private until U14 admission.`);
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
