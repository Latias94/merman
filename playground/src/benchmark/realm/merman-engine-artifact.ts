import mermanWasmUrl from "@mermanjs/web/pkg/merman_wasm_bg.wasm?url";
import engineSource from "../../../.runtime/benchmark-merman-engine.js?raw";
import engineManifest from "../../../.runtime/benchmark-merman-engine.json";

import type { RealmEngineArtifact } from "../../runtime/realm/channel-protocol.ts";

export function createMermanBenchmarkEngineArtifact(): RealmEngineArtifact {
  return Object.freeze({
    bytes: engineManifest.bytes,
    id: "benchmark-merman",
    resourceUrl: new URL(mermanWasmUrl, window.location.href).href,
    schemaVersion: engineManifest.schemaVersion as 1,
    sha256: engineManifest.sha256,
    source: engineSource,
  });
}
