import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

import { nativePackageName, resolveNodeTarget } from "../src/native-loader.mjs";
import { resolveInstalledEntrypoint } from "./smoke-installed-package.mjs";

const args = process.argv.slice(2);
const projectIndex = args.indexOf("--project");
const project = projectIndex === -1 ? null : args[projectIndex + 1];
const requireDirectSvg = args.includes("--require-direct-svg");
if (!project) {
  throw new Error("usage: node napi-task-smoke.mjs --project <installed-project> [--require-direct-svg]");
}

const entrypoint = resolveInstalledEntrypoint(project);
const requireFromInstalled = createRequire(entrypoint);
const target = resolveNodeTarget();
const artifact = requireFromInstalled.resolve(nativePackageName(target));
const binding = requireFromInstalled(artifact);
const { NativeEngine } = binding;
assert.equal(typeof NativeEngine, "function");
const supportsDirectSvg = typeof NativeEngine.prototype.executeSvg === "function";
assert.equal(typeof NativeEngine.prototype.executeSvgSync === "function", supportsDirectSvg);
if (requireDirectSvg) assert.ok(supportsDirectSvg, "the addon must provide the direct SVG methods");
const manifest = JSON.parse(readFileSync(
  path.resolve(path.dirname(entrypoint), "..", "package.json"), "utf8",
));
const identity = JSON.parse(binding.transportIdentityJson());
assert.equal(identity.transport_kind, "napi");
assert.equal(identity.package_version, manifest.version);
const artifactSha256 = createHash("sha256").update(readFileSync(artifact)).digest("hex");
const source = "flowchart TD\nA[Cold] --> B[Shared]";
const options = JSON.stringify({
  runtime_policy: "deterministic",
  site_config: { htmlLabels: false, flowchart: { look: "classic" } },
  svg: { diagram_id: "napi-task-smoke" },
});
const request = JSON.stringify({ operation_id: "svg", source, uri: null });
const cold = new NativeEngine(options);
const warm = new NativeEngine(options);
const disposing = new NativeEngine(options);
let accepted = [];

const publicOptionsJson = JSON.stringify({
  site_config: { htmlLabels: false },
  svg: { diagram_id: "napi-task-smoke-public" },
});
const publicRequest = {
  operationId: "svg", source: 'flowchart TD\nA["你好 & 🧪"] --> B', optionsJson: publicOptionsJson,
};

try {
  const concurrent = await Promise.all(Array.from({ length: 4 }, () => cold.execute(request)));
  const baseline = assertSvg(concurrent[0]);
  for (const response of concurrent) assert.equal(assertSvg(response), baseline);
  for (let index = 0; index < 2; index += 1) {
    assert.equal(assertSvg(await cold.execute(request)), baseline);
  }

  assert.equal(assertSvg(warm.executeSync(request)), baseline);
  assert.equal(assertSvg(await warm.execute(request)), baseline);

  const overrideRequest = JSON.stringify({
    operation_id: "svg", source, uri: null,
    options_json: JSON.stringify({ svg: { diagram_id: "napi-task-override" } }),
  });
  const overridden = assertSvg(await cold.execute(overrideRequest));
  assert.match(overridden, /<svg\b[^>]*\bid="napi-task-override"/);
  assert.notEqual(overridden, baseline);
  assert.equal(assertSvg(await cold.execute(request)), baseline);

  const limited = assertError(await cold.execute(JSON.stringify({
    operation_id: "svg", source, uri: null,
    options_json: JSON.stringify({ resources: { limits: { max_source_bytes: 1 } } }),
  })), "MERMAN_RESOURCE_LIMIT_EXCEEDED");
  assert.equal(limited.details.resource.limit_id, "max_source_bytes");
  assert.equal(limited.details.resource.max, 1);
  const malformed = assertError(await cold.execute(JSON.stringify({
    operation_id: "svg", source: "flowchart TD\nA --> ]", uri: null,
  })), "MERMAN_PARSE_ERROR");
  assert.equal(malformed.details.diagnostic.code, "merman.parse.diagram_parse");
  assert.equal(malformed.details.diagnostic.diagram_type, "flowchart-v2");
  assert.equal(assertSvg(await cold.execute(request)), baseline);

  const deadlineRequest = JSON.stringify({
    operation_id: "svg", source, uri: null, operation_control: { timeout_ms: 0 },
  });
  const [deadline, unaffected] = await Promise.all([
    cold.execute(deadlineRequest, undefined, 0), cold.execute(request),
  ]);
  const expired = assertError(deadline, "MERMAN_CANCELLED");
  assert.equal(expired.details.cancellation.reason, "deadline_exceeded");
  assert.equal(expired.details.cancellation.phase, "admission");
  assert.equal(assertSvg(unaffected), baseline);

  const controller = new AbortController();
  const pending = cold.execute(JSON.stringify({
    operation_id: "svg", uri: null,
    source: "flowchart TD\n" + Array.from(
      { length: 8_000 }, (_, index) => `N${index} --> N${index + 1}`,
    ).join("\n"),
  }), controller.signal);
  controller.abort();
  const cancelled = assertError(await pending, "MERMAN_CANCELLED");
  assert.equal(cancelled.details.cancellation.reason, "requested");
  assert.equal(assertSvg(await cold.execute(request)), baseline);

  const installed = await import(pathToFileURL(entrypoint));
  const publicEngine = await installed.createNodeEngine({ concurrency: 1, maxQueue: 1 });
  try {
    const svg = publicEngine.executeOperationSync(publicRequest);
    assert.equal(svg.operation_id, "svg");
    assert.equal(svg.media_type, "image/svg+xml");
    assert.equal(JSON.parse(svg.metadata_json).byte_length, Buffer.byteLength(svg.data));
    assert.match(svg.data, /<svg\b[^>]*\bid="napi-task-smoke-public"/);
    assert.ok(svg.data.includes("你好") && svg.data.includes("🧪"));

    // Disable the general methods after collecting the reference so accidental wire fallback
    // cannot make the direct-path smoke pass, even when both paths return identical SVG.
    const { execute, executeSync } = NativeEngine.prototype;
    if (supportsDirectSvg) {
      NativeEngine.prototype.execute = NativeEngine.prototype.executeSync = () => {
        throw new Error("general execution called by the public SVG helper");
      };
    }
    const { source: publicSource, optionsJson } = publicRequest;
    try {
      const first = publicEngine.renderSvg(publicSource, { optionsJson });
      const second = publicEngine.renderSvg(publicSource, { optionsJson });
      assert.throws(() => publicEngine.renderSvgSync(publicSource), installed.MermanLifecycleError);
      await assert.rejects(publicEngine.renderSvg(publicSource), installed.MermanQueueSaturatedError);
      for (const result of await Promise.all([first, second])) assert.equal(result, svg.data);
      assert.equal(publicEngine.renderSvgSync(publicSource, { optionsJson }), svg.data);

      const isDeadline = (error) => error instanceof installed.MermanOperationError &&
        error.codeName === "MERMAN_CANCELLED" &&
        error.cancellationDetails?.reason === "deadline_exceeded" &&
        error.cancellationDetails?.phase === "admission";
      assert.throws(() => publicEngine.renderSvgSync(publicSource, { timeoutMs: 0 }), isDeadline);
      await assert.rejects(publicEngine.renderSvg(publicSource, { timeoutMs: 0 }), isDeadline);
      await assert.rejects(publicEngine.renderSvg("flowchart TD\nA --> ]"), (error) =>
        error instanceof installed.MermanOperationError &&
        error.codeName === "MERMAN_PARSE_ERROR" &&
        error.diagnosticDetails?.code === "merman.parse.diagram_parse",
      );

      const publicAbort = new AbortController();
      const running = publicEngine.renderSvg("flowchart TD\n" + Array.from(
        { length: 8_000 }, (_, index) => `N${index} --> N${index + 1}`,
      ).join("\n"), { signal: publicAbort.signal });
      await Promise.resolve(); // The executor invokes the transport in the preceding microtask.
      publicAbort.abort();
      await assert.rejects(running, (error) => error instanceof installed.MermanOperationError &&
        error.codeName === "MERMAN_CANCELLED" && error.cancellationDetails?.reason === "requested");
      assert.equal(await publicEngine.renderSvg(publicSource, { optionsJson }), svg.data);
    } finally {
      if (supportsDirectSvg) Object.assign(NativeEngine.prototype, { execute, executeSync });
    }
  } finally {
    await publicEngine.dispose();
  }

  accepted = [disposing.execute(request), disposing.execute(request)];
  assert.doesNotThrow(
    () => disposing.dispose(),
    "dispose must release the owner while accepted tasks keep their engine alive",
  );
  disposing.dispose();
  for (const invoke of [
    () => disposing.execute(request),
    () => disposing.executeSync(request),
    () => disposing.runtimeCatalogJson(),
    () => disposing.metadataJson("supported-diagrams"),
  ]) {
    await assert.rejects(async () => invoke(), (error) => {
      const envelope = assertError(error.message, "MERMAN_INVALID_ARGUMENT");
      assert.match(envelope.message, /disposed/i);
      return true;
    });
  }
  for (const response of await Promise.all(accepted)) assert.equal(assertSvg(response), baseline);

  console.log(JSON.stringify({
    package: manifest.name, version: manifest.version, target,
    artifact, artifact_sha256: artifactSha256, node_version: process.version,
    cold_concurrent_requests: 4, cold_repeated_requests: 2,
    sync_warmup: true, override_recovery: true, failure_recovery: true,
    cancellation_isolation: true, deadline_isolation: true,
    accepted_after_dispose: 2, installed_public_async_requests: 2,
    installed_public_direct_svg: supportsDirectSvg,
  }));
} finally {
  await Promise.allSettled(accepted);
  cold.dispose();
  warm.dispose();
  disposing.dispose();
}

function assertSvg(response) {
  const envelope = JSON.parse(response);
  assert.equal(envelope.version, 1);
  assert.equal(envelope.ok, true, response);
  assert.equal(envelope.result.operation_id, "svg");
  assert.equal(envelope.result.media_type, "image/svg+xml");
  assert.match(envelope.result.data, /<svg\b/);
  assert.match(envelope.result.data, /Cold/);
  assert.match(envelope.result.data, /Shared/);
  return envelope.result.data;
}

function assertError(response, codeName) {
  const envelope = JSON.parse(response);
  assert.equal(envelope.version, 1);
  assert.equal(envelope.ok, false, response);
  assert.equal(envelope.error.code_name, codeName);
  return envelope.error;
}
