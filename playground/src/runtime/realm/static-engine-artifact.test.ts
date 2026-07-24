import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import test from "node:test";

import { REALM_BUDGETS } from "./channel-protocol.ts";
import {
  createStaticRealmEngineArtifact,
  type StaticEngineArtifactEnvironment,
} from "./static-engine-artifact.ts";

const source = "export const value = 1;";
const manifest = Object.freeze({
  bytes: Buffer.byteLength(source),
  id: "mermaid",
  schemaVersion: 1,
  sha256: createHash("sha256").update(source).digest("hex"),
});

test("static engine artifacts bind a same-origin response to the generated manifest", async () => {
  const artifact = await createStaticRealmEngineArtifact(
    {
      manifest,
      resourceUrl: null,
      sourceUrl: "assets/mermaid-engine.js",
    },
    environment(() => response(source))
  );

  assert.deepEqual(artifact, { ...manifest, resourceUrl: null, source });
  assert(Object.isFrozen(artifact));
});

test("static engine artifacts preserve UTF-8 split across response chunks", async () => {
  const unicodeSource = "export const value = 'A😀B';";
  const bytes = new TextEncoder().encode(unicodeSource);
  const split = bytes.indexOf(0xf0) + 2;
  const streamed = new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        controller.enqueue(bytes.subarray(0, split));
        controller.enqueue(bytes.subarray(split));
        controller.close();
      },
    }),
    { headers: { "content-length": String(bytes.byteLength) } }
  );

  const artifact = await createStaticRealmEngineArtifact(
    {
      manifest: engineManifest(unicodeSource),
      resourceUrl: null,
      sourceUrl: "assets/mermaid-engine.js",
    },
    environment(() => streamed)
  );
  assert.equal(artifact.source, unicodeSource);
});

test("static engine artifacts reject foreign origins and manifest drift", async () => {
  let fetchCalls = 0;
  const host = environment(() => {
    fetchCalls += 1;
    return response(source);
  });
  await assert.rejects(
    createStaticRealmEngineArtifact(
      {
        manifest,
        resourceUrl: null,
        sourceUrl: "https://other.example/mermaid-engine.js",
      },
      host
    ),
    /must be same-origin/
  );
  assert.equal(fetchCalls, 0);

  await assert.rejects(
    createStaticRealmEngineArtifact(
      {
        manifest: { ...manifest, bytes: manifest.bytes + 1 },
        resourceUrl: null,
        sourceUrl: "assets/mermaid-engine.js",
      },
      host
    ),
    /byte length is invalid/
  );
});

test("static engine artifacts enforce the transport budget before reading", async () => {
  const oversized = new Response(source, {
    headers: {
      "content-length": String(REALM_BUDGETS.engineArtifactBytes + 1),
    },
  });
  await assert.rejects(
    createStaticRealmEngineArtifact(
      {
        manifest,
        resourceUrl: null,
        sourceUrl: "assets/mermaid-engine.js",
      },
      environment(() => oversized)
    ),
    /exceeds its byte budget/
  );
});

test("static engine artifact acquisition obeys caller cancellation", async () => {
  const controller = new AbortController();
  controller.abort(new Error("artifact acquisition cancelled"));
  let fetchCalls = 0;

  await assert.rejects(
    createStaticRealmEngineArtifact(
      {
        manifest,
        resourceUrl: null,
        signal: controller.signal,
        sourceUrl: "assets/mermaid-engine.js",
      },
      environment(() => {
        fetchCalls += 1;
        return response(source);
      })
    ),
    /artifact acquisition cancelled/
  );
  assert.equal(fetchCalls, 0);
});

test("static engine artifact acquisition has a bounded fetch stage", async () => {
  await assert.rejects(
    createStaticRealmEngineArtifact(
      {
        manifest,
        resourceUrl: null,
        sourceUrl: "assets/mermaid-engine.js",
        timeoutMs: 1,
      },
      environment(
        (_url, init) =>
          new Promise((_resolve, reject) => {
            init.signal.addEventListener(
              "abort",
              () => reject(init.signal.reason),
              { once: true }
            );
          })
      )
    ),
    /request timed out/
  );
});

function environment(
  fetchResponse: (
    url: URL,
    init: Readonly<{ cache: "default"; signal: AbortSignal }>
  ) => Response | Promise<Response>
): StaticEngineArtifactEnvironment {
  return {
    async fetch(input, init) {
      return fetchResponse(input, init);
    },
    location: {
      href: "https://playground.example/merman/",
      origin: "https://playground.example",
    },
  };
}

function response(body: string): Response {
  return new Response(body, {
    headers: { "content-length": String(Buffer.byteLength(body)) },
  });
}

function engineManifest(body: string) {
  return {
    bytes: Buffer.byteLength(body),
    id: "mermaid",
    schemaVersion: 1,
    sha256: createHash("sha256").update(body).digest("hex"),
  };
}
