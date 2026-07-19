import {
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  validateRealmHello,
  type RealmBootIdentity,
  type RealmEngineArtifactIdentity,
} from "../../runtime/realm/channel-protocol.ts";
import { startBenchmarkRealm } from "./bootstrap.ts";
import engineManifest from "../../../.runtime/benchmark-merman-engine.json";
import "./benchmark-realm.css";

const boot = readBootIdentity();
void startBenchmarkRealm(
  boot,
  engineManifest as RealmEngineArtifactIdentity,
  "merman"
);

function readBootIdentity(): RealmBootIdentity {
  const params = new URLSearchParams(window.location.hash.slice(1));
  if (
    params.size !== 4 ||
    params.get("protocol") !== String(REALM_PROTOCOL_VERSION) ||
    params.get("kind") !== "benchmark"
  ) {
    throw new RealmProtocolError("Benchmark realm boot fragment is invalid.");
  }
  const realmId = params.get("realm");
  const bootNonce = params.get("boot");
  if (!realmId || !bootNonce) {
    throw new RealmProtocolError("Benchmark realm boot identity is invalid.");
  }
  const boot: RealmBootIdentity = {
    kind: "benchmark",
    realmId,
    bootNonce,
  };
  validateRealmHello(
    { type: "realm-hello", protocol: REALM_PROTOCOL_VERSION, ...boot },
    boot
  );
  return boot;
}
