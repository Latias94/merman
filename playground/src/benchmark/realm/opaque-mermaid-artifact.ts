import { BENCHMARK_MERMAID_ARTIFACT_PROJECTION } from "./generated/benchmark-mermaid.generated.ts";
import type {
  RealmBootIdentity,
  RealmEngineArtifact,
} from "../../runtime/realm/channel-protocol.ts";
import {
  createProjectedOpaqueRealmDocument,
  createProjectedRealmEngineArtifact,
} from "../../runtime/realm/opaque-realm-projection.ts";

export function createBenchmarkMermaidEngineArtifact(
  signal: AbortSignal,
): Promise<RealmEngineArtifact> {
  return createProjectedRealmEngineArtifact(
    BENCHMARK_MERMAID_ARTIFACT_PROJECTION.engine,
    signal,
  );
}

export function createOpaqueMermaidBenchmarkRealmDocument(
  boot: RealmBootIdentity,
): string {
  return createProjectedOpaqueRealmDocument(
    BENCHMARK_MERMAID_ARTIFACT_PROJECTION,
    boot,
  );
}
