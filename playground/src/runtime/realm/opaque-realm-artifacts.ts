import compareBootstrapSource from "../../../.runtime/opaque-compare-bootstrap.js?raw";
import compareBootstrapManifest from "../../../.runtime/opaque-compare-bootstrap.json";
import benchmarkBootstrapSource from "../../../.runtime/opaque-benchmark-mermaid-bootstrap.js?raw";
import benchmarkBootstrapManifest from "../../../.runtime/opaque-benchmark-mermaid-bootstrap.json";
import compareEngineSource from "../../../.runtime/compare-mermaid-engine.js?raw";
import compareEngineManifest from "../../../.runtime/compare-mermaid-engine.json";
import benchmarkEngineSource from "../../../.runtime/benchmark-mermaid-engine.js?raw";
import benchmarkEngineManifest from "../../../.runtime/benchmark-mermaid-engine.json";

import type {
  RealmBootIdentity,
  RealmEngineArtifact,
  RealmEngineArtifactId,
} from "./channel-protocol.ts";
import {
  buildOpaqueRealmDocument,
  type OpaqueRealmScriptArtifact,
} from "./opaque-realm-document.ts";

const compareBootstrap = bootstrapArtifact(
  compareBootstrapManifest,
  compareBootstrapSource
);
const benchmarkBootstrap = bootstrapArtifact(
  benchmarkBootstrapManifest,
  benchmarkBootstrapSource
);

export const compareMermaidEngineArtifact = engineArtifact(
  compareEngineManifest,
  compareEngineSource
);
export const benchmarkMermaidEngineArtifact = engineArtifact(
  benchmarkEngineManifest,
  benchmarkEngineSource
);

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

function engineArtifact(
  manifest: typeof compareEngineManifest,
  source: string
): RealmEngineArtifact {
  return Object.freeze({
    bytes: manifest.bytes,
    id: manifest.id as RealmEngineArtifactId,
    resourceUrl: null,
    schemaVersion: manifest.schemaVersion as 1,
    sha256: manifest.sha256,
    source,
  });
}
