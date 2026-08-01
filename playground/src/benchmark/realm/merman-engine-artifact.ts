import { MERMAN_WASM_URL } from "@mermanjs/web";
import engineManifest from "../../../.runtime/benchmark-merman-engine.json";

import type { RealmEngineArtifact } from "../../runtime/realm/channel-protocol.ts";
import { createStaticRealmEngineArtifact } from "../../runtime/realm/static-engine-artifact.ts";

export function createMermanBenchmarkEngineArtifact(
  signal: AbortSignal
): Promise<RealmEngineArtifact> {
  return createStaticRealmEngineArtifact(
    {
      manifest: engineManifest,
      resourceUrl: new URL(MERMAN_WASM_URL, window.location.href).href,
      signal,
      sourceUrl: `${import.meta.env.BASE_URL}opaque-realm/benchmark-merman-engine.js?sha256=${engineManifest.sha256}`,
    }
  );
}
