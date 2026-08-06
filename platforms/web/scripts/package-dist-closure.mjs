import { existsSync } from "node:fs";
import path from "node:path";

import {
  collectStaticModuleGraph,
  relativeModuleFiles,
} from "./static-module-graph.mjs";
import {
  surfaceModuleOwners,
  webPackages,
} from "./surface-manifest.mjs";

const WASM_GLUE_SPECIFIER = "../../artifacts/wasm/merman_wasm.js";

export function packageDistClosure(distRoot, packageId) {
  const runtime = packageRuntimeDistClosure(distRoot, packageId);
  const { root } = runtime;
  const entryRoot = path.join(root, "package-entries");
  const declarations = collectStaticModuleGraph({
    entry: path.join(entryRoot, `${packageId}.d.ts`),
    root,
    mode: "declaration",
  });

  assertClosedPackageGraph(declarations, packageId, "declaration");
  if (declarations.dynamicImports.length !== 0) {
    throw new Error(`${packageId} declaration closure must not use dynamic imports.`);
  }

  const declarationModules = relativeModuleFiles(declarations);
  const moduleFiles = uniqueSorted([
    ...runtime.javascriptModules,
    ...declarationModules,
  ]);
  const files = [...moduleFiles];
  for (const relative of moduleFiles) {
    const map = `${relative}.map`;
    if (existsSync(path.join(root, ...map.split("/")))) files.push(map);
  }

  return Object.freeze({
    ...runtime,
    files: Object.freeze(uniqueSorted(files)),
    declarationModules: Object.freeze(declarationModules),
  });
}

export function packageRuntimeDistClosure(distRoot, packageId) {
  const root = path.resolve(distRoot);
  const entryRoot = path.join(root, "package-entries");
  const descriptor = webPackages.find(({ id }) => id === packageId);
  if (!descriptor) {
    throw new Error(`Unknown Web package ID: ${packageId}.`);
  }
  const javascript = collectStaticModuleGraph({
    entry: path.join(entryRoot, `${packageId}.js`),
    root,
    mode: "runtime",
  });
  assertClosedPackageGraph(javascript, packageId, "JavaScript");
  assertDeclaredSurfaceModuleGraph(javascript, descriptor);
  const dynamicImports = javascript.dynamicImports;
  if (
    dynamicImports.length !== 1 ||
    dynamicImports[0] !== WASM_GLUE_SPECIFIER
  ) {
    throw new Error(
      `${packageId} JavaScript closure must dynamically import only ${WASM_GLUE_SPECIFIER}.`,
    );
  }
  const javascriptModules = relativeModuleFiles(javascript);
  return Object.freeze({
    root,
    packageId,
    javascriptModules: Object.freeze(javascriptModules),
  });
}

export function assertDeclaredSurfaceModuleGraph(graph, descriptor) {
  const allowedOwners = new Set(
    [...descriptor.runtimeExportModules, ...descriptor.valueExportModules].map(
      ({ specifier }) => moduleOwner(specifier, descriptor.id),
    ),
  );
  const ownedFiles = relativeModuleFiles(graph).filter(
    (relative) =>
      relative !== `package-entries/${descriptor.id}.js` &&
      relative !== `package-entries/${descriptor.id}.ts`,
  );
  const unknownModules = ownedFiles.filter(
    (relative) => !Object.hasOwn(surfaceModuleOwners, ownerSpecifier(relative)),
  );
  if (unknownModules.length !== 0) {
    throw new Error(
      `${descriptor.id} runtime closure contains modules without an explicit owner: ${unknownModules.join(", ")}.`,
    );
  }
  const foreignModules = ownedFiles.filter(
    (relative) =>
      !allowedOwners.has(surfaceModuleOwners[ownerSpecifier(relative)]),
  );
  if (foreignModules.length !== 0) {
    throw new Error(
      `${descriptor.id} runtime closure contains modules declared for another surface: ${foreignModules.join(", ")}.`,
    );
  }
}

function assertClosedPackageGraph(graph, packageId, label) {
  if (graph.externalImports.length !== 0) {
    const imports = graph.externalImports.join(", ");
    throw new Error(
      `${packageId} ${label} closure has undeclared external imports: ${imports}.`,
    );
  }
  const siblingEntries = relativeModuleFiles(graph).filter(
    (relative) =>
      relative.startsWith("package-entries/") &&
      relative !== `package-entries/${packageId}.js` &&
      relative !== `package-entries/${packageId}.d.ts`,
  );
  if (siblingEntries.length !== 0) {
    throw new Error(
      `${packageId} ${label} closure reaches sibling package entries: ${siblingEntries.join(", ")}.`,
    );
  }
}

function moduleOwner(specifier, packageId) {
  const owner = surfaceModuleOwners[specifier];
  if (!owner) {
    throw new Error(
      `${packageId} package surface references a module without an explicit owner: ${specifier}.`,
    );
  }
  return owner;
}

function ownerSpecifier(relative) {
  return `../${relative.replace(/\.ts$/, ".js")}`;
}

function uniqueSorted(values) {
  return [...new Set(values)].sort();
}
