import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { promisify } from "node:util";
import { expect, test } from "@playwright/test";
import type { HostTextMeasureRequest, MountedDiagram } from "../../platforms/web/packages/full/dist/public-types.js";

type MountApi = typeof import("../../platforms/web/packages/full/dist/package-entries/full.js");
type FullModule = Awaited<ReturnType<MountApi["initMerman"]>>;
declare global {
  interface Window {
    mountFixture: { api: MountApi; prepared: number; freed: number; renders: number; mount?: MountedDiagram; target?: HTMLElement; fontLoaded?: boolean; fontEvents?: number };
  }
}
const repositoryRoot = path.resolve(import.meta.dirname, "../..");
const packageRoot = path.join(repositoryRoot, "platforms/web/packages/full");
const kanban = "kanban\n  todo[Todo]\n    a[First]\n  doing[Doing]\n    b[Second]\n  done[Done]\n    c[Third]";

test.beforeAll(async () => {
  await promisify(execFile)(process.execPath, ["platforms/web/scripts/verify-wasm-inputs.mjs", "--package", "full"], { cwd: repositoryRoot });
  const { assertArtifactFileEvidence } = await import(pathToFileURL(path.join(repositoryRoot, "platforms/web/scripts/prepack-check.mjs")).href);
  const provenance = JSON.parse(await readFile(path.join(packageRoot, "artifacts/provenance.json"), "utf8"));
  assertArtifactFileEvidence({
    packageWasmRoot: path.join(packageRoot, "artifacts/wasm"), sourceWasmRoot: path.join(repositoryRoot, "platforms/web/pkg/full"),
    packageDistRoot: path.join(packageRoot, "dist"), sourceDistRoot: path.join(repositoryRoot, "platforms/web/dist"),
    packageId: "full", artifactFiles: provenance.artifact_files, label: "persistent mount public browser package",
  });
});

test.beforeEach(async ({ page }) => {
  await page.route("**/__mounted_diagram/**", async route => {
    const relative = new URL(route.request().url()).pathname.split("/__mounted_diagram/")[1];
    if (relative === "" || relative === "frame") {
      await route.fulfill({ contentType: "text/html", body: "<!doctype html><meta name='viewport' content='width=device-width, initial-scale=1'><body style='margin:0'></body>" }); return;
    }
    if (relative === "font.ttf") {
      await route.fulfill({ contentType: "font/ttf", body: await readFile(path.join(repositoryRoot, "crates/merman-export/tests/fonts/Cousine-Regular.ttf")) }); return;
    }
    const root = relative.startsWith("render/") ? path.join(repositoryRoot, "platforms/web/packages/render") : packageRoot;
    const file = path.resolve(root, relative.startsWith("render/") ? relative.slice(7) : relative);
    expect(file.startsWith(root + path.sep)).toBe(true);
    await route.fulfill({ contentType: file.endsWith(".wasm") ? "application/wasm" : "text/javascript", body: await readFile(file) });
  });
  await page.goto("./__mounted_diagram/");
  await page.evaluate(async () => {
    const api: MountApi = await import(new URL("dist/package-entries/full.js", location.href).href);
    const fixture = window.mountFixture = { api, prepared: 0, freed: 0, renders: 0 };
    await api.initMerman({ loader: async () => {
      const module: FullModule = await import(new URL("artifacts/wasm/merman_wasm.js", location.href).href);
      const Native = module.PreparedSource!;
      return { ...module, PreparedSource: class extends Native {
        constructor(...args: ConstructorParameters<typeof Native>) { super(...args); fixture.prepared++; }
        free() { fixture.freed++; super.free(); }
      } };
    } });
  });
});

test("fit-width resize restores complete columns without reparsing or height-only redraw", async ({ page }) => {
  await page.evaluate(source => {
    const f = window.mountFixture; const target = f.target = document.createElement("div");
    target.style.cssText = "width:800px;padding:13px;border:3px solid black;box-sizing:content-box";
    document.body.append(target);
    f.mount = f.api.mountDiagram(target, source, { outputOptions: { layout: { adaptation: "fit-width" } }, onRender: () => f.renders++ });
  }, kanban);
  await expect.poll(() => page.evaluate(() => window.mountFixture.mount?.result?.metadata.layout_adaptation?.requested_width)).toBe(800);
  const wide = await page.evaluate(() => window.mountFixture.mount!.result!.svg);
  const narrowWidth = await page.evaluate(() => Math.min(425, document.documentElement.clientWidth - 32));
  await page.evaluate(width => { window.mountFixture.target!.style.width = `${width}px`; }, narrowWidth);
  await expect.poll(() => page.evaluate(() => window.mountFixture.mount?.result?.metadata.layout_adaptation?.requested_width)).toBe(narrowWidth);
  expect(await page.evaluate(() => window.mountFixture.prepared)).toBe(1);
  expect(await page.evaluate(() => window.mountFixture.target!.textContent)).toContain("Third");
  await page.screenshot({ path: test.info().outputPath("mounted-narrow.png") });
  const count = await page.evaluate(() => window.mountFixture.renders);
  await page.evaluate(() => { window.mountFixture.target!.style.height = "900px"; });
  await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  expect(await page.evaluate(() => window.mountFixture.renders)).toBe(count);
  await page.evaluate(() => { window.mountFixture.target!.style.width = "800px"; });
  await expect.poll(() => page.evaluate(() => window.mountFixture.mount!.result!.svg)).toBe(wide);
  await page.evaluate(() => { window.mountFixture.target!.style.width = `${Math.min(800, document.documentElement.clientWidth - 32)}px`; });
  await expect.poll(() => page.evaluate(() => window.mountFixture.mount!.result!.metadata.layout_adaptation!.requested_width)).toBe(await page.evaluate(() => Math.min(800, document.documentElement.clientWidth - 32)));
  await page.screenshot({ path: test.info().outputPath("mounted-responsive.png") });
});

test("failed render requests retain the last good DOM and preserve typed errors", async ({ page }) => {
  const result = await page.evaluate(() => {
    const f = window.mountFixture; const target = document.body.appendChild(document.createElement("div"));
    const mount = f.api.mountDiagram(target, "flowchart LR\nA[Good]-->B");
    const good = target.firstChild; const errors = [];
    const deterministic = mount.result!.svg === f.api.renderSvg("flowchart LR\nA[Good]-->B", { svg: { diagram_id: target.querySelector("svg")!.id } });
    mount.update("flowchart LR\nA -->"); errors.push(mount.error);
    const parseRetained = target.firstChild === good;
    mount.update("flowchart LR\nA[Good]-->B", { outputOptions: { layout: { adaptation: "fit-width" } } }); errors.push(mount.error);
    const layoutRetained = target.firstChild === good;
    mount.update("flowchart LR\nA[Good]-->B", { outputOptions: { resources: { limits: { max_svg_bytes: 1 } } } }); errors.push(mount.error);
    const resourceRetained = target.firstChild === good;
    mount.update("flowchart LR\nA[Good]-->B", { outputOptions: { timeout_ms: 0 } }); errors.push(mount.error);
    mount.update("flowchart LR\nA[Good]-->B", { prepareOptions: { environment: { text_measurement: "deterministic" } }, textMeasurement: "browser" }); errors.push(mount.error);
    const failedStatus = mount.status;
    const unsafeSource = 'flowchart LR\nA[Unsafe]-->B\nclick A href "javascript:alert(1)"';
    const unsafeOptions = { site_config: { securityLevel: "loose" } };
    mount.update(unsafeSource, { prepareOptions: unsafeOptions });
    const securityFailure = { status: mount.status, retained: target.firstChild === good, error: mount.error };
    mount.update("flowchart LR\nA[Recovered]-->B", {});
    const text = target.textContent; mount.destroy();
    return { deterministic, parseRetained, layoutRetained, resourceRetained, failedStatus, securityFailure, errors, text, freed: f.freed, prepared: f.prepared };
  });
  expect(result.deterministic).toBe(true); expect(result.parseRetained).toBe(true); expect(result.layoutRetained).toBe(true); expect(result.resourceRetained).toBe(true);
  await test.info().attach("mount-failure-results.json", { body: JSON.stringify(result, null, 2), contentType: "application/json" });
  expect(result.failedStatus).toBe("failed");
  expect(result.securityFailure.status).toBe("failed"); expect(result.securityFailure.retained).toBe(true);
  expect(result.securityFailure.error).toMatchObject({ code_name: "MERMAN_OPTIONS_JSON_ERROR", message: expect.stringContaining("site_config.securityLevel") });
  expect(result.errors.map(error => (error as { code_name: string }).code_name)).toEqual(["MERMAN_PARSE_ERROR", "MERMAN_RENDER_ERROR", "MERMAN_RESOURCE_LIMIT_EXCEEDED", "MERMAN_CANCELLED", "MERMAN_INVALID_ARGUMENT"]);
  expect(result.text).toContain("Recovered"); expect(result.freed).toBe(result.prepared);
});

test("hidden mounts retain new source and resume when measurable", async ({ page }) => {
  await page.evaluate(() => {
    const f = window.mountFixture; const target = f.target = document.createElement("div"); target.style.display = "none"; document.body.append(target);
    f.mount = f.api.mountDiagram(target, "flowchart LR\nA[Old]-->B"); f.mount.update("flowchart LR\nA[Newest]-->B");
  });
  expect(await page.evaluate(() => [window.mountFixture.mount!.status, window.mountFixture.prepared])).toEqual(["pending", 0]);
  await page.evaluate(() => { window.mountFixture.target!.style.display = "block"; });
  await expect.poll(() => page.evaluate(() => window.mountFixture.mount!.status)).toBe("ready");
  expect(await page.evaluate(() => window.mountFixture.target!.textContent)).toContain("Newest");
  expect(await page.evaluate(() => window.mountFixture.prepared)).toBe(1);
  await page.evaluate(() => {
    const f = window.mountFixture; f.target!.style.visibility = "hidden";
    f.mount!.update("flowchart LR\nA[AfterVisibility]-->B");
  });
  expect(await page.evaluate(() => [window.mountFixture.mount!.status, window.mountFixture.prepared])).toEqual(["pending", 1]);
  await page.evaluate(() => { const f = window.mountFixture; f.target!.style.visibility = "visible"; f.mount!.refresh(); });
  expect(await page.evaluate(() => window.mountFixture.target!.textContent)).toContain("AfterVisibility");
});

test("real font loading refreshes browser metrics on the same prepared source", async ({ page }) => {
  await page.evaluate(async () => {
    const f = window.mountFixture; const target = f.target = document.body.appendChild(document.createElement("div"));
    f.fontEvents = 0; document.fonts.addEventListener("loadingdone", () => f.fontEvents!++);
    f.mount = f.api.mountDiagram(target, "flowchart LR\nA[WWWW iiiii changing font width]-->B", {
      prepareOptions: { site_config: { fontFamily: "MountedCousine, serif", htmlLabels: false } }, textMeasurement: "browser", onRender: () => f.renders++,
    });
    await document.fonts.ready;
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
  });
  const before = await page.evaluate(() => ({ svg: window.mountFixture.mount!.result!.svg, renders: window.mountFixture.renders }));
  await page.evaluate(async () => {
    const font = new FontFace("MountedCousine", `url(${new URL("font.ttf", location.href).href})`);
    document.fonts.add(font); await font.load(); await document.fonts.ready; window.mountFixture.fontLoaded = true;
  });
  await expect.poll(() => page.evaluate(() => window.mountFixture.renders)).toBeGreaterThan(before.renders);
  expect(await page.evaluate(() => window.mountFixture.fontEvents)).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.mountFixture.prepared)).toBe(1);
  expect(await page.evaluate(() => window.mountFixture.mount!.result!.svg)).not.toBe(before.svg);
  await page.evaluate(() => window.mountFixture.mount!.destroy());
  expect(await page.evaluate(() => document.querySelectorAll("[data-merman-text-measure-probe]").length)).toBe(0);
});

test("measurement and publication callback reentry cannot commit stale results", async ({ page }) => {
  await page.evaluate(() => {
    const f = window.mountFixture; const target = f.target = document.body.appendChild(document.createElement("div"));
    const mount = f.mount = f.api.mountDiagram(target, "flowchart LR\nA[First]-->B");
    let interrupted = false;
    mount.update("flowchart LR\nA[Stale]-->B", { textMeasurement: () => {
      if (!interrupted) { interrupted = true; mount.update("flowchart LR\nA[Newest]-->B", {}); } return undefined;
    }, onRender: () => { throw new Error("stale callback ran"); } });
  });
  await expect.poll(() => page.evaluate(() => window.mountFixture.target!.textContent)).toContain("Newest");
  const result = await page.evaluate(() => {
    const f = window.mountFixture; const mount = f.mount!; const target = f.target!;
    mount.update("flowchart LR\nA[Callback]-->B", { onRender: (_result, controller) => { controller.update("flowchart LR\nA[NewerCallback]-->B", {}); throw new Error("old callback failure"); } });
    const callbackStatus = mount.status; const callbackError = mount.error;
    mount.update("flowchart LR\nA[ThrowAfterPublish]-->B", { onRender: () => { throw new Error("callback failed after publication"); } });
    const callbackRetained = target.textContent?.includes("ThrowAfterPublish") && mount.result?.svg.includes("ThrowAfterPublish");
    mount.update("flowchart LR\nA[DestroyMe]-->B", { textMeasurement: () => { mount.destroy(); return undefined; } });
    const destroyedText = target.textContent; const old = mount;
    const replacement = f.api.mountDiagram(target, "flowchart LR\nA[Replacement]-->B", { textMeasurement: "browser" });
    replacement.destroy(); old.destroy();
    return { callbackStatus, callbackError, callbackRetained, destroyedText, replacement: target.textContent, prepared: f.prepared, freed: f.freed };
  });
  expect(result.callbackStatus).toBe("ready"); expect(result.callbackError).toBeNull();
  expect(result.callbackRetained).toBe(true); expect(result.destroyedText).toContain("ThrowAfterPublish"); expect(result.replacement).toContain("Replacement");
  expect(result.freed).toBe(result.prepared);
});

test("adoption binds probes to the new Document and keeps navigation/base URI admission", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const f = window.mountFixture; const target = document.body.appendChild(document.createElement("div"));
    const source = 'flowchart LR\nA[<b>First<br/>Second</b>]-->B\nclick A "https://example.com/ticket"';
    const mount = f.api.mountDiagram(target, source, { textMeasurement: "browser" });
    const frame = document.createElement("iframe"); frame.src = new URL("frame", location.href).href;
    document.body.append(frame); await new Promise(resolve => frame.addEventListener("load", resolve, { once: true }));
    const other = frame.contentDocument!; other.body.appendChild(other.adoptNode(target)); mount.refresh();
    const oldProbes = document.querySelectorAll("[data-merman-text-measure-probe]").length;
    const newProbes = other.querySelectorAll("[data-merman-text-measure-probe]").length;
    const root = target.firstChild; const anchor = target.querySelector("a");
    const base = other.createElement("base"); base.href = "https://collector.example/external.svg"; other.head.appendChild(base);
    mount.refresh(); const rejected = String((mount.error as { message?: string })?.message ?? mount.error);
    const retained = target.firstChild === root;
    base.remove(); mount.refresh(); mount.destroy();
    return { oldProbes, newProbes, prepared: f.prepared, rejected, retained, owner: root!.ownerDocument === other, target: anchor?.getAttribute("target"), rel: anchor?.getAttribute("rel"), remainingProbes: other.querySelectorAll("[data-merman-text-measure-probe]").length };
  });
  expect(result.owner).toBe(true); expect(result.oldProbes).toBe(0); expect(result.newProbes).toBe(3);
  expect(result.prepared).toBe(1); expect(result.rejected).toMatch(/base URI differs/); expect(result.retained).toBe(true);
  expect(result.target).toBe("_blank"); expect(result.rel).toContain("noopener"); expect(result.rel).toContain("noreferrer"); expect(result.remainingProbes).toBe(0);
});

test("separately loaded package instances have independent mount ID namespaces", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const f = window.mountFixture; const a = document.body.appendChild(document.createElement("div"));
    const first = f.api.mountDiagram(a, "flowchart LR\nA-->B");
    const other: MountApi = await import(new URL("render/dist/package-entries/render.js", location.href).href);
    await other.initMerman();
    const b = document.body.appendChild(document.createElement("div")); const second = other.mountDiagram(b, "flowchart LR\nA-->B");
    const ids = [a.querySelector("svg")!.id, b.querySelector("svg")!.id];
    const all = Array.from(document.querySelectorAll("[id]"), element => element.id);
    first.destroy(); second.destroy(); return { ids, unique: new Set(all).size === all.length };
  });
  expect(result.ids[0]).not.toBe(result.ids[1]); expect(result.ids.every(value => /^[a-z][a-zA-Z0-9_-]+$/.test(value))).toBe(true); expect(result.unique).toBe(true);
});

for (const look of ["classic", "neo", "handDrawn"] as const) {
  test(`mounted Step edges preserve browser timing, isolation and reduced motion with ${look}`, async ({ page }) => {
    const result = await page.evaluate(look => {
      const fixture = window.mountFixture;
      const source = `---
config:
  htmlLabels: false
  look: ${look}
  handDrawnSeed: 42
  flowchart:
    stepDuration: 0.25
    stepNumbers: true
---
flowchart LR
  A e1@--> B
  A e2@--> C
  B e3@-->|Later| D
  e1@{ step: 1, animate: true }
  e2@{ step: 1 }
  e3@{ step: 5 }`;
      const targets = [0, 1].map(() => {
        const target = document.body.appendChild(document.createElement("div"));
        target.style.width = "800px";
        return target;
      });
      const mounts = targets.map(target => fixture.api.mountDiagram(target, source));
      const statuses = mounts.map(mount => mount.status);
      const sample = (root: SVGSVGElement, time: number) => {
        root.pauseAnimations();
        root.setCurrentTime(time);
        return Array.from(root.querySelectorAll<SVGGraphicsElement>(".edge-step-dot"), dot => {
          const matrix = dot.getCTM()!;
          return { step: Number(dot.getAttribute("data-step")), visible: Math.hypot(matrix.a, matrix.b) > 1e-6, x: matrix.e, y: matrix.f };
        });
      };
      const roots = targets.map(target => target.querySelector("svg")!);
      const samples = roots.map(root => [0.125, 0.175, 0.625, 1.125, 1.375].map(time => sample(root, time)));
      const identities = Array.from(document.querySelectorAll("[id]"), element => element.id);
      const styles = roots.map(root => root.querySelectorAll(".edge-step-style").length);
      const badges = roots.map(root => Array.from(root.querySelectorAll(".edge-step-badge"), badge => badge.textContent));
      const secondBefore = sample(roots[1], 1.125);
      const secondTimeBefore = roots[1].getCurrentTime();
      mounts[0].update("flowchart LR\nA[Plain] --> B");
      const plainHasNoStep = !targets[0].querySelector("animateMotion, .edge-step-dot, .edge-step-highlight, .edge-step-style");
      mounts[0].update(source);
      const reused = sample(targets[0].querySelector("svg")!, 0.125);
      const secondTimeAfter = roots[1].getCurrentTime();
      const secondAfter = sample(roots[1], secondTimeAfter);
      const updatedIdentities = Array.from(document.querySelectorAll("[id]"), element => element.id);
      const finalStatuses = mounts.map(mount => mount.status);
      for (const mount of mounts) mount.destroy();
      return { statuses, samples, styles, badges, plainHasNoStep, reused, secondBefore, secondAfter, secondTimeBefore, secondTimeAfter, finalStatuses, unique: new Set(identities).size === identities.length, uniqueAfterUpdate: new Set(updatedIdentities).size === updatedIdentities.length, prepared: fixture.prepared, freed: fixture.freed };
    }, look);

    expect(result.statuses).toEqual(["ready", "ready"]);
    expect(result.finalStatuses).toEqual(["ready", "ready"]);
    expect(result.styles).toEqual([1, 1]);
    expect(result.badges).toEqual([["1", "1", "5"], ["1", "1", "5"]]);
    expect(result.unique).toBe(true);
    expect(result.uniqueAfterUpdate).toBe(true);
    expect(result.secondTimeAfter).toBe(result.secondTimeBefore);
    expect(result.secondAfter).toEqual(result.secondBefore);
    expect(result.plainHasNoStep).toBe(true);
    expect(result.freed).toBe(result.prepared);
    for (const samples of result.samples) {
      expect(samples.map(dots => dots.filter(dot => dot.visible).map(dot => dot.step))).toEqual([[1, 1], [1, 1], [], [5], [1, 1]]);
      expect(Math.hypot(samples[1][0].x - samples[0][0].x, samples[1][0].y - samples[0][0].y)).toBeGreaterThan(1e-6);
    }
    expect(result.reused.filter(dot => dot.visible).map(dot => dot.step)).toEqual([1, 1]);

    await page.emulateMedia({ reducedMotion: "reduce" });
    await expect.poll(() => page.locator(".edge-step-dot, .edge-step-highlight").evaluateAll(elements =>
      elements.length === 12 && elements.every(element => getComputedStyle(element).display === "none"),
    )).toBe(true);
    await expect.poll(() => page.locator(".edge-step-badge").evaluateAll(elements =>
      elements.length === 6 && elements.every(element => getComputedStyle(element).display !== "none"),
    )).toBe(true);
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await expect.poll(() => page.locator(".edge-step-dot, .edge-step-highlight").evaluateAll(elements =>
      elements.every(element => getComputedStyle(element).display !== "none"),
    )).toBe(true);
  });
}


test("explicit Document measurement creates every primitive in that Document", async ({ page }) => {
  const result = await page.evaluate(async () => {
    const f = window.mountFixture; const frame = document.createElement("iframe");
    frame.src = new URL("frame", location.href).href; document.body.append(frame);
    await new Promise(resolve => frame.addEventListener("load", resolve, { once: true }));
    const other = frame.contentDocument!;
    const session = f.api.createBrowserTextMeasurementSession(other);
    const request: HostTextMeasureRequest = {
      operation: "measure", phase: "svg-bbox", text: "Two<br/>lines wrapping words", font_family: "serif", font_size: 16,
      font_weight: "normal", font_style: "normal", max_width: 60, has_max_width: true, line_height: 24,
      letter_spacing: 0, word_spacing: 0, wrap_mode: "svg-like", direction: "auto", white_space: "normal",
    };
    const values = [
      session.measure({ ...request, operation: "canvas-measure-text-width", phase: "layout" }),
      session.measure({ ...request, operation: "wrapped", phase: "wrap" }),
      session.measure({ ...request, operation: "create-text-middle-bbox-y-offset" }),
      session.measure({ ...request, operation: "wrapped-with-raw-width", phase: "wrap", wrap_mode: "html-like", white_space: "break-spaces" }),
    ];
    const probes = other.querySelectorAll("[data-merman-text-measure-probe]").length;
    const ambient = document.querySelectorAll("[data-merman-text-measure-probe]").length;
    const dynamicBreaks = other.querySelectorAll("[data-merman-text-measure-probe] br").length;
    session.dispose();
    return { handled: values.every(Boolean), probes, ambient, dynamicBreaks, remaining: other.querySelectorAll("[data-merman-text-measure-probe]").length };
  });
  expect(result.handled).toBe(true); expect(result.probes).toBe(3); expect(result.ambient).toBe(0); expect(result.dynamicBreaks).toBeGreaterThan(0); expect(result.remaining).toBe(0);
});


test("host custom-element disconnection can remount without an older publication winning", async ({ page }) => {
  const result = await page.evaluate(() => {
    const f = window.mountFixture; const target = document.body.appendChild(document.createElement("div"));
    let replacement: MountedDiagram | null = null;
    let staleCallback = false;
    customElements.define("mount-disconnect-probe", class extends HTMLElement {
      disconnectedCallback() { replacement = f.api.mountDiagram(target, "flowchart LR\nA[ReplacementFromDOM]-->B"); }
    });
    target.appendChild(document.createElement("mount-disconnect-probe"));
    const superseded = f.api.mountDiagram(target, "flowchart LR\nA[Obsolete]-->B", { onRender: () => { staleCallback = true; } });
    const text = target.textContent; const status = superseded.status;
    (replacement as MountedDiagram | null)?.destroy(); superseded.destroy();
    return { text, status, staleCallback, prepared: f.prepared, freed: f.freed };
  });
  expect(result.text).toContain("ReplacementFromDOM"); expect(result.status).toBe("destroyed"); expect(result.staleCallback).toBe(false); expect(result.freed).toBe(result.prepared);
});

test("measurement callback remount restores nested callbacks and defers the old native free", async ({ page }) => {
  const results = await page.evaluate(async () => {
    const run = (api: Pick<MountApi, "mountDiagram">, counters: { prepared: number; freed: number }) => {
      const target = document.body.appendChild(document.createElement("div"));
      const callbacks: string[] = [];
      const errors: unknown[] = [];
      const events: string[] = [];
      let replacement: MountedDiagram | null = null;
      let remounted = false;
      let innerCalls = 0;
      let outerResumedCalls = 0;
      let freedInsideCallback = -1;
      const oldSource = "flowchart LR\nA[Old outer first label]-->B[Old outer second label]-->C[Old outer third label]";
      const old = api.mountDiagram(target, oldSource);
      old.update(oldSource, {
        textMeasurement: () => {
          if (!remounted) {
            remounted = true;
            events.push("outer-enter");
            replacement = api.mountDiagram(target, "flowchart LR\nA[New nested first label]-->B[New nested second label]", {
              textMeasurement: () => { innerCalls++; events.push("inner-measure"); return undefined; },
              onRender: () => { callbacks.push("new"); events.push("new-render"); },
              onError: error => errors.push(error),
            });
            freedInsideCallback = counters.freed;
            events.push("outer-return");
          } else {
            outerResumedCalls++;
            events.push("outer-resumed");
          }
          return undefined;
        },
        onRender: () => callbacks.push("old"),
        onError: error => errors.push(error),
      });
      const next = replacement as MountedDiagram | null;
      const oldStatus = old.status;
      const nextStatus = next?.status;
      const textAfterOuter = target.textContent;
      const freedAfterOuter = counters.freed;
      old.destroy();
      next?.refresh();
      const textAfterRefresh = target.textContent;
      const preparedBeforeDestroy = counters.prepared;
      next?.destroy(); old.destroy();
      return {
        oldStatus, nextStatus, textAfterOuter, textAfterRefresh, freedInsideCallback,
        freedAfterOuter, preparedBeforeDestroy, freedAfterDestroy: counters.freed,
        innerCalls, outerResumedCalls, callbacks, errors, events,
      };
    };
    const results = [{ packageId: "full", ...run(window.mountFixture.api, window.mountFixture) }];
    const renderApi: typeof import("../../platforms/web/packages/render/dist/package-entries/render.js") =
      await import(new URL("render/dist/package-entries/render.js", location.href).href);
    const counters = { prepared: 0, freed: 0 };
    await renderApi.initMerman({ loader: async () => {
      const module: Awaited<ReturnType<typeof renderApi.initMerman>> =
        await import(new URL("render/artifacts/wasm/merman_wasm.js", location.href).href);
      const Native = module.PreparedSource;
      return { ...module, PreparedSource: class extends Native {
        constructor(...args: ConstructorParameters<typeof Native>) { super(...args); counters.prepared++; }
        free() { counters.freed++; super.free(); }
      } };
    } });
    results.push({ packageId: "render", ...run(renderApi, counters) });
    return results;
  });
  await test.info().attach("measurement-remount-results.json", { body: JSON.stringify(results, null, 2), contentType: "application/json" });
  for (const result of results) {
    expect(result.oldStatus, result.packageId).toBe("destroyed");
    expect(result.nextStatus, result.packageId).toBe("ready");
    expect(result.textAfterOuter, result.packageId).toContain("New nested first label");
    expect(result.textAfterOuter, result.packageId).not.toContain("Old outer");
    expect(result.textAfterRefresh, result.packageId).toContain("New nested second label");
    expect(result.freedInsideCallback, result.packageId).toBe(0);
    expect(result.freedAfterOuter, result.packageId).toBe(1);
    expect(result.preparedBeforeDestroy, result.packageId).toBe(2);
    expect(result.freedAfterDestroy, result.packageId).toBe(2);
    expect(result.innerCalls, result.packageId).toBeGreaterThan(0);
    expect(result.outerResumedCalls, result.packageId).toBeGreaterThan(0);
    expect(result.callbacks, result.packageId).toEqual(["new", "new"]);
    expect(result.errors, result.packageId).toEqual([]);
    expect(result.events.indexOf("new-render"), result.packageId).toBeLessThan(result.events.indexOf("outer-return"));
    expect(result.events.indexOf("outer-return"), result.packageId).toBeLessThan(result.events.indexOf("outer-resumed"));
  }
});
