import assert from "node:assert/strict";
import test from "node:test";

import { applyOpaqueRealmSourcePolicy } from "./opaque-realm-source-policy.mjs";

const FONT_FACE = String.raw`
  @font-face{font-family:MS Sans Serif;src:url(/fonts/MS%20Sans%20Serif.ttf) format(\"truetype\")}
`.trim();
const MERMAID_ARTIFACT = { id: "mermaid", resourcePolicy: "none-v1" };

test("Mermaid opaque source drops ZenUML's unused optional font resource", () => {
  const source = `before${FONT_FACE}.zenuml{font-family:Helvetica,Verdana,serif}after`;

  assert.equal(
    applyOpaqueRealmSourcePolicy(MERMAID_ARTIFACT, source),
    "before.zenuml{font-family:Helvetica,Verdana,serif}after",
  );
});

test("Mermaid opaque source fails closed when the ZenUML injection drifts", () => {
  assert.throws(
    () => applyOpaqueRealmSourcePolicy(MERMAID_ARTIFACT, "no font injection"),
    /expected one ZenUML optional font injection; found 0/u,
  );
  assert.throws(
    () =>
      applyOpaqueRealmSourcePolicy(
        MERMAID_ARTIFACT,
        `${FONT_FACE}${FONT_FACE}`,
      ),
    /expected one ZenUML optional font injection; found 2/u,
  );
});

test("other opaque artifacts are unchanged", () => {
  const source = `before${FONT_FACE}after`;

  assert.equal(
    applyOpaqueRealmSourcePolicy(
      { id: "benchmark-merman", resourcePolicy: "same-origin-wasm-v1" },
      source,
    ),
    source,
  );
});
