import { readFileSync } from "node:fs";
import path from "node:path";

import { isCspHash } from "./csp-policy.mjs";
import {
  OPAQUE_REALM_ARTIFACT_PLAN,
  realmForKey,
} from "./opaque-realm-artifact-plan.mjs";

const PAGE_CONTRACTS = Object.freeze(
  Object.fromEntries(
    OPAQUE_REALM_ARTIFACT_PLAN.pages.map((page) => [
      page.source,
      Object.freeze(
        page.inlineRealms.map((realmKey) => {
          const realm = realmForKey(OPAQUE_REALM_ARTIFACT_PLAN, realmKey);
          return Object.freeze({
            manifest: `${realm.bootstrap.outputBase}.json`,
            placeholder: realm.bootstrap.cspPlaceholder,
          });
        }),
      ),
    ]),
  ),
);

export function loadOpaqueRealmCspHashes(playgroundRoot) {
  const manifestHashes = new Map();
  return Object.fromEntries(
    Object.entries(PAGE_CONTRACTS).map(([fileName, contracts]) => [
      fileName,
      Object.freeze(
        contracts.map((contract) =>
          loadManifestHash(
            playgroundRoot,
            fileName,
            contract.manifest,
            manifestHashes,
          ),
        ),
      ),
    ]),
  );
}

export function injectOpaqueRealmCspHashes(fileName, html, hashes) {
  const contracts = PAGE_CONTRACTS[fileName];
  if (!contracts) return html;
  const pageHashes = hashes[fileName];
  if (
    !Array.isArray(pageHashes) ||
    pageHashes.length !== contracts.length ||
    pageHashes.some((hash) => !isCspHash(hash)) ||
    new Set(pageHashes).size !== pageHashes.length
  ) {
    throw new Error(`${fileName} has an invalid opaque-realm CSP hash set.`);
  }

  let transformed = html;
  for (const [index, contract] of contracts.entries()) {
    const occurrences = transformed.split(contract.placeholder).length - 1;
    if (occurrences !== 1) {
      throw new Error(
        `${fileName} must contain exactly one ${contract.placeholder} placeholder; found ${occurrences}.`,
      );
    }
    transformed = transformed.replace(contract.placeholder, pageHashes[index]);
  }
  const residual = transformed.match(/__MERMAN_[A-Z0-9_]+_CSP_HASH__/gu);
  if (residual) {
    throw new Error(
      `${fileName} contains undeclared opaque-realm CSP placeholders: ${residual.join(", ")}.`,
    );
  }
  return transformed;
}

export function createOpaqueRealmCspPlugin(hashes) {
  return {
    name: "merman-opaque-realm-csp",
    enforce: "pre",
    transformIndexHtml(html, context) {
      return injectOpaqueRealmCspHashes(
        path.basename(context.filename),
        html,
        hashes,
      );
    },
  };
}

function loadManifestHash(playgroundRoot, fileName, manifestFile, cache) {
  const manifestPath = path.join(
    playgroundRoot,
    OPAQUE_REALM_ARTIFACT_PLAN.roots.generated,
    manifestFile,
  );
  const cached = cache.get(manifestPath);
  if (cached) return cached;
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    throw new Error(
      `Cannot read the ${fileName} opaque-realm CSP manifest at ${manifestPath}: ${errorMessage(error)}`,
      { cause: error },
    );
  }
  if (
    manifest?.schemaVersion !== 1 ||
    typeof manifest.cspHash !== "string" ||
    !isCspHash(manifest.cspHash)
  ) {
    throw new Error(
      `${manifestFile} does not contain a valid schema-1 CSP hash.`,
    );
  }
  cache.set(manifestPath, manifest.cspHash);
  return manifest.cspHash;
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}
