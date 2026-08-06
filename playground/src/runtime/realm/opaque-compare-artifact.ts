import { COMPARE_MERMAID_ARTIFACT_PROJECTION } from "./generated/compare-mermaid.generated.ts";
import type {
  RealmBootIdentity,
  RealmEngineArtifact,
} from "./channel-protocol.ts";
import {
  createProjectedOpaqueRealmDocument,
  createProjectedRealmEngineArtifact,
} from "./opaque-realm-projection.ts";

export function createCompareMermaidEngineArtifact(
  signal: AbortSignal,
): Promise<RealmEngineArtifact> {
  return createProjectedRealmEngineArtifact(
    COMPARE_MERMAID_ARTIFACT_PROJECTION.engine,
    signal,
  );
}

export function createOpaqueCompareRealmDocument(
  boot: RealmBootIdentity,
): string {
  return createProjectedOpaqueRealmDocument(
    COMPARE_MERMAID_ARTIFACT_PROJECTION,
    boot,
  );
}
