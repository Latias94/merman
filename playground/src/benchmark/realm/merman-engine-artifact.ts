import { MERMAN_WASM_URL } from "@mermanjs/web";

import { BENCHMARK_MERMAN_ARTIFACT_PROJECTION } from "./generated/benchmark-merman.generated.ts";

import type { RealmEngineArtifact } from "../../runtime/realm/channel-protocol.ts";
import { createProjectedRealmEngineArtifact } from "../../runtime/realm/opaque-realm-projection.ts";

export function createMermanBenchmarkEngineArtifact(
  signal: AbortSignal
): Promise<RealmEngineArtifact> {
  return createProjectedRealmEngineArtifact(
    BENCHMARK_MERMAN_ARTIFACT_PROJECTION,
    signal,
    new URL(MERMAN_WASM_URL, window.location.href).href,
  );
}
