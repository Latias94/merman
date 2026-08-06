import path from "node:path";
import { fileURLToPath } from "node:url";

import { OPAQUE_REALM_ARTIFACT_PLAN } from "./opaque-realm-artifact-plan.mjs";
import { verifyPreparedOpaqueRealmArtifacts } from "./opaque-realm-artifact-verifier.mjs";

const playgroundRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

await verifyPreparedOpaqueRealmArtifacts(
  playgroundRoot,
  OPAQUE_REALM_ARTIFACT_PLAN,
);
