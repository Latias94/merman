import compareBootstrapSource from "../../../.runtime/opaque-compare-bootstrap.js?raw";
import compareBootstrapManifest from "../../../.runtime/opaque-compare-bootstrap.json";
import benchmarkBootstrapSource from "../../../.runtime/opaque-benchmark-mermaid-bootstrap.js?raw";
import benchmarkBootstrapManifest from "../../../.runtime/opaque-benchmark-mermaid-bootstrap.json";
import mermaidEngineManifest from "../../../.runtime/mermaid-engine.json";

import type {
  RealmBootIdentity,
  RealmEngineArtifact,
} from "./channel-protocol.ts";
import {
  buildOpaqueRealmDocument,
  type OpaqueRealmScriptArtifact,
} from "./opaque-realm-document.ts";
import { createStaticRealmEngineArtifact } from "./static-engine-artifact.ts";

const compareBootstrap = bootstrapArtifact(
  compareBootstrapManifest,
  compareBootstrapSource
);
const benchmarkBootstrap = bootstrapArtifact(
  benchmarkBootstrapManifest,
  benchmarkBootstrapSource
);
const OPAQUE_ENGINE_BASE_URL = `${import.meta.env.BASE_URL}opaque-realm/`;
const MERMAID_ENGINE_URL = `${OPAQUE_ENGINE_BASE_URL}mermaid-engine.js?sha256=${mermaidEngineManifest.sha256}`;

export function createCompareMermaidEngineArtifact(
  signal: AbortSignal
): Promise<RealmEngineArtifact> {
  return createStaticRealmEngineArtifact(
    {
      manifest: mermaidEngineManifest,
      resourceUrl: null,
      signal,
      sourceUrl: MERMAID_ENGINE_URL,
    }
  );
}

export function createBenchmarkMermaidEngineArtifact(
  signal: AbortSignal
): Promise<RealmEngineArtifact> {
  return createStaticRealmEngineArtifact(
    {
      manifest: mermaidEngineManifest,
      resourceUrl: null,
      signal,
      sourceUrl: MERMAID_ENGINE_URL,
    }
  );
}

export function createOpaqueCompareRealmDocument(
  boot: RealmBootIdentity
): string {
  return buildOpaqueRealmDocument(boot, compareBootstrap);
}

export function createOpaqueMermaidBenchmarkRealmDocument(
  boot: RealmBootIdentity
): string {
  return buildOpaqueRealmDocument(boot, benchmarkBootstrap);
}

function bootstrapArtifact(
  manifest: typeof compareBootstrapManifest,
  script: string
): OpaqueRealmScriptArtifact {
  return Object.freeze({
    bytes: manifest.bytes,
    cspHash: manifest.cspHash,
    id: manifest.id,
    schemaVersion: manifest.schemaVersion as 1,
    sha256: manifest.sha256,
    script,
  });
}
