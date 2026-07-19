import {
  REALM_PROTOCOL_VERSION,
  RealmProtocolError,
  validateRealmHello,
  type RealmBootIdentity,
} from "./channel-protocol.ts";

export interface OpaqueRealmScriptArtifact {
  readonly bytes: number;
  readonly cspHash: string;
  readonly id: string;
  readonly schemaVersion: 1;
  readonly script: string;
  readonly sha256: string;
}

const EXECUTION_STYLE = [
  "html,body{box-sizing:border-box;margin:0;width:100%;height:100%;overflow:hidden}",
  "#presentation-host{display:block;width:100%;height:100%;overflow:visible}",
].join("");

export function buildOpaqueRealmDocument(
  boot: RealmBootIdentity,
  artifact: OpaqueRealmScriptArtifact
): string {
  validateRealmHello(
    { type: "realm-hello", protocol: REALM_PROTOCOL_VERSION, ...boot },
    boot
  );
  if (
    artifact.schemaVersion !== 1 ||
    artifact.id !== boot.kind ||
    !Number.isSafeInteger(artifact.bytes) ||
    artifact.bytes <= 0 ||
    !/^sha256-[A-Za-z0-9+/]+={0,2}$/.test(artifact.cspHash) ||
    !/^[a-f0-9]{64}$/.test(artifact.sha256) ||
    !artifact.script ||
    /<\/script/i.test(artifact.script)
  ) {
    throw new RealmProtocolError("Opaque realm artifact is invalid.");
  }
  const csp = [
    "default-src 'none'",
    "base-uri 'none'",
    "connect-src 'none'",
    "font-src 'none'",
    "form-action 'none'",
    "frame-src 'none'",
    "img-src data:",
    "media-src 'none'",
    "object-src 'none'",
    `script-src '${artifact.cspHash}' blob:`,
    "style-src 'unsafe-inline'",
    "worker-src 'none'",
  ].join("; ");
  return [
    "<!doctype html><html><head><meta charset=\"utf-8\">",
    `<meta http-equiv="Content-Security-Policy" content="${csp}">`,
    bootMeta("merman-realm-kind", boot.kind),
    bootMeta("merman-realm-id", boot.realmId),
    bootMeta("merman-realm-boot", boot.bootNonce),
    `<style>${EXECUTION_STYLE}</style>`,
    "</head><body>",
    '<div id="presentation-host"></div>',
    `<script>${artifact.script}</script>`,
    "</body></html>",
  ].join("");
}

function bootMeta(name: string, value: string): string {
  return `<meta name="${name}" content="${value}">`;
}
