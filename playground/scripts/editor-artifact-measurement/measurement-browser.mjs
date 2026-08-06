import { artifactReadyAtMs } from "./measurement-server.mjs";

export async function measureSemanticEquivalence({
  baselines,
  build,
  chromium,
  headed,
  server,
}) {
  const browser = await chromium.launch({
    channel: "chromium",
    headless: !headed,
  });
  const browserVersion = browser.version();
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    await page.goto(new URL(build.equivalenceHtml, server.url).href, {
      waitUntil: "domcontentloaded",
      timeout: 60_000,
    });
    await page.waitForFunction(
      () => globalThis.__mermanEditorArtifactEquivalenceV1?.status === "ready",
      undefined,
      { timeout: 30_000 },
    );
    await page.evaluate((fixtures) => {
      globalThis.__runMermanEditorArtifactEquivalenceV1(fixtures);
    }, baselines);
    await page.waitForFunction(
      () => {
        const status =
          globalThis.__mermanEditorArtifactEquivalenceV1?.status ?? "missing";
        return status === "complete" || status === "error";
      },
      undefined,
      { timeout: 180_000 },
    );
    const state = await page.evaluate(
      () => globalThis.__mermanEditorArtifactEquivalenceV1,
    );
    await context.close();
    if (state.status === "error") {
      throw new Error(
        `${build.id} semantic-equivalence matrix failed: ${state.error.message}${state.error.stack ? `\n${state.error.stack}` : ""}`,
      );
    }
    if (state.status !== "complete") {
      throw new Error(
        `${build.id} semantic-equivalence matrix returned ${state.status}.`,
      );
    }
    return { browserVersion, matrix: state.matrix };
  } finally {
    await browser.close();
  }
}

export async function measureVariantPair({ build, chromium, headed, server }) {
  const browser = await chromium.launch({
    channel: "chromium",
    headless: !headed,
    args: ["--enable-precise-memory-info"],
  });
  const browserVersion = browser.version();
  try {
    const context = await browser.newContext();
    await context.addInitScript(installBrowserMeasurementInstrumentation);
    const page = await context.newPage();

    const cold = await measureNavigation({
      build,
      mode: "cold",
      page,
      server,
    });
    await page.goto("about:blank", { waitUntil: "load" });
    const warm = await measureNavigation({
      build,
      mode: "warm",
      page,
      server,
    });

    await context.close();
    return { browserVersion, cold, warm };
  } finally {
    await browser.close();
  }
}

async function measureNavigation({ build, mode, page, server }) {
  server.beginObservation();
  await page.goto(server.url, {
    waitUntil: "domcontentloaded",
    timeout: 60_000,
  });
  await page.waitForFunction(
    () => {
      const state = globalThis.__mermanEditorArtifactMeasurementV1;
      return (
        state &&
        (typeof state.firstDiagnosticsError === "string" ||
          (Number.isFinite(state.workerReadyAtMs) &&
            Number.isFinite(state.firstDiagnosticsAtMs) &&
            Number.isFinite(state.mainReadyAtMs) &&
            Number.isFinite(state.mainFirstResultAtMs)))
      );
    },
    undefined,
    { timeout: 60_000 },
  );
  await page.waitForTimeout(300);
  await page.evaluate(() => {
    globalThis.__mermanEditorArtifactMeasurementV1.stopMemory = true;
  });
  await page
    .waitForFunction(
      () => globalThis.__mermanEditorArtifactMeasurementV1?.memoryDone === true,
      undefined,
      { timeout: 5_000 },
    )
    .catch(() => undefined);
  const state = await page.evaluate(() => {
    const value = globalThis.__mermanEditorArtifactMeasurementV1;
    return {
      firstDiagnosticsError: value.firstDiagnosticsError,
      firstDiagnosticsAtMs: value.firstDiagnosticsAtMs,
      mainFirstResultAtMs: value.mainFirstResultAtMs,
      mainReadyAtMs: value.mainReadyAtMs,
      memoryErrors: value.memoryErrors,
      memorySamples: value.memorySamples,
      memoryScope: value.memoryScope,
      timeOrigin: performance.timeOrigin,
      workerReadyAtMs: value.workerReadyAtMs,
    };
  });
  const serverNetwork = server.endObservation();
  if (typeof state.firstDiagnosticsError === "string") {
    throw new Error(
      `${build.id} ${mode} first diagnostics failed: ${state.firstDiagnosticsError}.`,
    );
  }
  const mainArtifactReadyAtMs = artifactReadyAtMs(
    serverNetwork.requests,
    build.mainWasm.file,
    state.timeOrigin,
    mode,
  );
  const workerArtifactReadyAtMs = artifactReadyAtMs(
    serverNetwork.requests,
    build.workerWasm.file,
    state.timeOrigin,
    mode,
  );
  const memorySamples = state.memorySamples.filter(
    (sample) => Number.isFinite(sample.bytes) && sample.bytes >= 0,
  );
  if (memorySamples.length === 0 || typeof state.memoryScope !== "string") {
    throw new Error(
      `${build.id} ${mode} run returned no peak-memory samples: ${state.memoryErrors.join("; ") || "unknown reason"}.`,
    );
  }

  return {
    firstDiagnosticsMs: state.firstDiagnosticsAtMs,
    mainCompileInitializeMs: Math.max(
      0,
      state.mainReadyAtMs - mainArtifactReadyAtMs,
    ),
    mainFirstResultMs: state.mainFirstResultAtMs,
    network: serverNetwork,
    peakMemory: {
      bytes: Math.max(...memorySamples.map((sample) => sample.bytes)),
      samples: memorySamples,
      scope: state.memoryScope,
    },
    totalTransferBytes: serverNetwork.bodyBytes,
    workerCompileInitializeMs: Math.max(
      0,
      state.workerReadyAtMs - workerArtifactReadyAtMs,
    ),
    workerReadyMs: state.workerReadyAtMs,
  };
}

function installBrowserMeasurementInstrumentation() {
  const state = {
    firstDiagnosticsAtMs: null,
    firstDiagnosticsError: null,
    mainFirstResultAtMs: null,
    mainReadyAtMs: null,
    memoryDone: false,
    memoryErrors: [],
    memorySamples: [],
    memoryScope: "user-agent-specific-memory",
    stopMemory: false,
    workerReadyAtMs: null,
  };
  Object.defineProperty(globalThis, "__mermanEditorArtifactMeasurementV1", {
    configurable: false,
    enumerable: false,
    value: state,
    writable: false,
  });
  try {
    localStorage.setItem("merman-language", "en");
  } catch {
    // The measured HTTP document provides storage; opaque bootstrap documents may not.
  }

  const pending = new Map();
  const NativeWorker = globalThis.Worker;
  class MeasuredWorker extends NativeWorker {
    constructor(scriptURL, options) {
      super(scriptURL, options);
      this.__mermanMeasured = /merman-language\.worker/iu.test(
        String(scriptURL),
      );
      if (!this.__mermanMeasured) return;
      this.addEventListener("message", (event) => {
        const message = event.data;
        if (!message || typeof message !== "object") return;
        const request = pending.get(message.requestId);
        if (message.type === "ready" && request?.type === "initialize") {
          state.workerReadyAtMs ??= performance.now();
        }
        if (request?.type === "query" && request.kind === "diagnostics") {
          if (message.type === "queryResult") {
            state.firstDiagnosticsAtMs ??= performance.now();
          } else if (message.type === "error") {
            state.firstDiagnosticsError ??= `${String(message.code ?? "QUERY_FAILED")}: ${String(message.message ?? "unknown error")}`;
          }
        }
        if (Number.isSafeInteger(message.requestId))
          pending.delete(message.requestId);
      });
    }

    postMessage(message, transferOrOptions) {
      if (
        this.__mermanMeasured &&
        message &&
        typeof message === "object" &&
        Number.isSafeInteger(message.requestId)
      ) {
        pending.set(message.requestId, {
          kind: message.query?.kind ?? null,
          type: message.type,
        });
      }
      return arguments.length === 1
        ? super.postMessage(message)
        : super.postMessage(message, transferOrOptions);
    }
  }
  Object.defineProperty(globalThis, "Worker", {
    configurable: true,
    value: MeasuredWorker,
    writable: true,
  });

  const shadowRoots = new Set();
  const inspectPresentation = () => {
    if (
      state.mainReadyAtMs === null &&
      /WASM:\s*Ready\b/u.test(document.body?.innerText ?? "")
    ) {
      state.mainReadyAtMs = performance.now();
    }
    if (state.mainFirstResultAtMs !== null) return;
    for (const root of shadowRoots) {
      if (root.querySelector("svg")) {
        state.mainFirstResultAtMs = performance.now();
        return;
      }
    }
    for (const host of document.querySelectorAll(".preview-container > div")) {
      if (host.shadowRoot?.querySelector("svg")) {
        state.mainFirstResultAtMs = performance.now();
        return;
      }
    }
  };
  const observer = new MutationObserver(inspectPresentation);
  const nativeAttachShadow = Element.prototype.attachShadow;
  Element.prototype.attachShadow = function attachMeasuredShadow(init) {
    const root = nativeAttachShadow.call(this, init);
    shadowRoots.add(root);
    observer.observe(root, { childList: true, subtree: true });
    queueMicrotask(inspectPresentation);
    return root;
  };
  document.addEventListener(
    "DOMContentLoaded",
    () => {
      observer.observe(document.documentElement, {
        childList: true,
        subtree: true,
      });
      inspectPresentation();
    },
    { once: true },
  );

  const sampleMemory = async () => {
    if (typeof performance.measureUserAgentSpecificMemory !== "function") {
      state.memoryErrors.push("measureUserAgentSpecificMemory is unavailable");
      return;
    }
    try {
      const measured = await performance.measureUserAgentSpecificMemory();
      state.memorySamples.push({
        atMs: performance.now(),
        bytes: measured.bytes,
      });
    } catch (error) {
      state.memoryErrors.push(String(error));
    }
  };
  void (async () => {
    while (!state.stopMemory && performance.now() < 60_000) {
      await sampleMemory();
      if (state.stopMemory) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    if (state.memorySamples.length === 0) await sampleMemory();
    state.memoryDone = true;
  })();
}
