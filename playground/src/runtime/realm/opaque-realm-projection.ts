import {
  isRealmEngineArtifactId,
  REALM_ENGINE_RESOURCE_POLICIES,
  type RealmEngineArtifactId,
} from "./generated/opaque-realm-plan.generated.ts";
import type {
  RealmBootIdentity,
  RealmEngineArtifact,
  RealmKind,
} from "./channel-protocol.ts";
import {
  buildOpaqueRealmDocument,
  type OpaqueRealmScriptArtifact,
} from "./opaque-realm-document.ts";
import {
  createStaticRealmEngineArtifact,
  type StaticEngineArtifactRequest,
} from "./static-engine-artifact.ts";

type ResourcePolicy =
  (typeof REALM_ENGINE_RESOURCE_POLICIES)[RealmEngineArtifactId];

interface GeneratedBootstrapManifest {
  readonly bytes: unknown;
  readonly cspHash: unknown;
  readonly engineArtifact: unknown;
  readonly id: unknown;
  readonly schemaVersion: unknown;
  readonly sha256: unknown;
}

export interface RealmEngineArtifactProjection {
  readonly manifest: StaticEngineArtifactRequest["manifest"];
  readonly publicPath: string;
  readonly resourcePolicy: ResourcePolicy;
}

export interface OpaqueRealmArtifactProjection {
  readonly bootstrap: {
    readonly manifest: GeneratedBootstrapManifest;
    readonly source: string;
  };
  readonly engine: RealmEngineArtifactProjection;
  readonly realm: {
    readonly key: string;
    readonly kind: RealmKind;
  };
}

export function createProjectedRealmEngineArtifact(
  projection: RealmEngineArtifactProjection,
  signal: AbortSignal,
  resourceUrl: string | null = null,
): Promise<RealmEngineArtifact> {
  const id = projection.manifest.id;
  if (
    !isRealmEngineArtifactId(id) ||
    projection.resourcePolicy !== REALM_ENGINE_RESOURCE_POLICIES[id]
  ) {
    throw new Error("Realm engine artifact projection is inconsistent.");
  }
  if (
    (projection.resourcePolicy === "none-v1" && resourceUrl !== null) ||
    (projection.resourcePolicy === "same-origin-wasm-v1" &&
      !isSameOriginResource(resourceUrl))
  ) {
    throw new Error("Realm engine resource policy is invalid.");
  }
  return createStaticRealmEngineArtifact({
    manifest: projection.manifest,
    resourceUrl,
    signal,
    sourceUrl: `${import.meta.env.BASE_URL}${projection.publicPath}?sha256=${String(projection.manifest.sha256)}`,
  });
}

export function createProjectedOpaqueRealmDocument(
  projection: OpaqueRealmArtifactProjection,
  boot: RealmBootIdentity,
): string {
  if (
    projection.realm.kind !== boot.kind ||
    !sameEngineIdentity(
      projection.bootstrap.manifest.engineArtifact,
      projection.engine.manifest,
    )
  ) {
    throw new Error("Opaque realm browser projection is inconsistent.");
  }
  return buildOpaqueRealmDocument(
    boot,
    projectBootstrapArtifact(projection.bootstrap),
  );
}

function projectBootstrapArtifact(
  bootstrap: OpaqueRealmArtifactProjection["bootstrap"],
): OpaqueRealmScriptArtifact {
  const manifest = bootstrap.manifest;
  return Object.freeze({
    bytes: manifest.bytes as number,
    cspHash: manifest.cspHash as string,
    id: manifest.id as string,
    schemaVersion: manifest.schemaVersion as 1,
    sha256: manifest.sha256 as string,
    script: bootstrap.source,
  });
}

function sameEngineIdentity(
  actual: unknown,
  expected: StaticEngineArtifactRequest["manifest"],
): boolean {
  if (typeof actual !== "object" || actual === null || Array.isArray(actual)) {
    return false;
  }
  const identity = actual as Record<string, unknown>;
  return (
    Object.keys(identity).length === 4 &&
    identity.schemaVersion === expected.schemaVersion &&
    identity.id === expected.id &&
    identity.bytes === expected.bytes &&
    identity.sha256 === expected.sha256
  );
}

function isSameOriginResource(value: string | null): value is string {
  if (!value) return false;
  try {
    return new URL(value, window.location.href).origin === window.location.origin;
  } catch {
    return false;
  }
}
