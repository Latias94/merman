import {
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  validateRealmHello,
  type RealmBootIdentity,
  type RealmEngineArtifactIdentity,
} from "./channel-protocol.ts";
import { startCompareRealm } from "./compare-bootstrap.ts";

declare const __MERMAN_ENGINE_ARTIFACT_IDENTITY__: RealmEngineArtifactIdentity;

const boot = readBootIdentity();
void startCompareRealm(boot, __MERMAN_ENGINE_ARTIFACT_IDENTITY__);

function readBootIdentity(): RealmBootIdentity {
  const kind = readMeta("merman-realm-kind");
  const realmId = readMeta("merman-realm-id");
  const bootNonce = readMeta("merman-realm-boot");
  if (kind !== "compare") {
    throw new RealmProtocolError("Opaque Compare realm kind is invalid.");
  }
  const boot: RealmBootIdentity = { kind, realmId, bootNonce };
  validateRealmHello(
    {
      type: "realm-hello",
      protocol: REALM_PROTOCOL_VERSION,
      ...boot,
    },
    boot
  );
  return boot;
}

function readMeta(name: string): string {
  const value = document
    .querySelector<HTMLMetaElement>(`meta[name="${name}"]`)
    ?.content.trim();
  if (!value) throw new RealmProtocolError(`Opaque realm ${name} is missing.`);
  return value;
}
