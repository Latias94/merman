import { expect, test, type Locator, type Page } from "@playwright/test";
import { decodeShareHash, encodeShareHash } from "../src/lib/share";
import { DEFAULT_WORKSPACE_SNAPSHOT } from "../src/lib/workspace-snapshot";
import { monitorBrowserErrors, replaceEditorSource, replaceMermaidConfig, waitForPreviewSvg } from "./helpers/playground";

async function openPanel(page: Page) {
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const panel = page.getByRole("region", { name: "Appearance", exact: true });
  await expect(panel).toBeVisible();
  return panel;
}

async function start(page: Page, mermaidConfig = "{}", themePresetId: string | null = null,
  code = 'flowchart LR\nA[Authored source] --> B\n%%{init: {"flowchart":{"look":"handDrawn"}}}%%') {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.addInitScript(() => {
    if (window === window.top && (location.protocol === "http:" || location.protocol === "https:")) localStorage.setItem("merman-language", "en");
  });
  const workspace = {
    ...DEFAULT_WORKSPACE_SNAPSHOT,
    code,
    mermaidConfig, themePresetId,
  };
  await page.goto(`./${encodeShareHash(workspace)}`, { waitUntil: "domcontentloaded" });
  await waitForPreviewSvg(page);
  return workspace;
}

async function workspaceFromLink(page: Page) {
  await page.getByRole("button", { name: "Share", exact: true }).click();
  const copy = page.getByRole("menuitem", { name: "Copy workspace link", exact: true });
  await expect(copy).toBeEnabled();
  await copy.click();
  const url = await page.evaluate(() => navigator.clipboard.readText());
  return decodeShareHash(new URL(url).hash)!;
}

async function svg(page: Page) {
  return page.locator(".preview-container > div").first().evaluate((host) =>
    host.shadowRoot?.querySelector("svg")?.outerHTML ?? "",
  );
}

async function openAdvanced(panel: Locator) {
  const advanced = panel.getByTestId("theme-advanced");
  if (await advanced.getAttribute("open") === null) {
    await advanced.locator("summary").first().click();
  }
}

async function expectBrutalistDepth(page: Page, root: Locator): Promise<void> {
  const facts = await root.evaluate((svg) => {
    const faces = [...svg.querySelectorAll<SVGGElement>("g[data-et='node']")];
    const depths = [...svg.querySelectorAll<SVGGElement>("[data-merman-theme-depth='node']")];
    const edges = [...svg.querySelectorAll("path.flowchart-link")];
    const geometry = (body: SVGGraphicsElement) => [body, ...body.querySelectorAll<SVGGraphicsElement>("*")]
      .map((element) => ({
        tag: element.localName,
        attributes: ["class", "x", "y", "width", "height", "rx", "ry", "cx", "cy", "r", "points", "d", "transform"]
          .map((name) => element.getAttribute(name)),
        paint: ["fill", "stroke", "stroke-width", "opacity", "fill-opacity", "stroke-opacity", "rx", "ry"]
          .map((name) => getComputedStyle(element).getPropertyValue(name)),
      }));
    return {
      faces: faces.map((face) => ({
        id: face.getAttribute("data-id"), look: face.getAttribute("data-look"),
        bodies: face.querySelectorAll(":scope > .label-container").length,
        tag: face.querySelector(".label-container")?.localName,
        children: [...(face.querySelector(".label-container")?.children ?? [])].map((element) => element.localName),
      })),
      edges: edges.length,
      depths: depths.map((depth) => {
        const body = depth.querySelector<SVGGraphicsElement>(":scope > .label-container")!;
        const binding = body.getAttribute("filter")!;
        const id = /^url\(#([^)]*)\)$/u.exec(binding)?.[1];
        const owners = faces.filter((face) => id?.startsWith(`${face.id}-theme-effect-`));
        const owner = owners[0];
        const face = owner?.querySelector<SVGGraphicsElement>(":scope > .label-container");
        const graphs = id ? [...svg.querySelectorAll<SVGFilterElement>(`filter[id="${id}"]`)] : [];
        const graph = graphs[0];
        const merge = graph?.lastElementChild;
        const matrix = (element: SVGGraphicsElement) => {
          const ctm = element.getCTM()!;
          return [ctm.a, ctm.b, ctm.c, ctm.d, ctm.e, ctm.f];
        };
        return {
          owner: owner?.getAttribute("data-id"), owners: owners.length, look: depth.getAttribute("data-look"),
          hidden: depth.getAttribute("aria-hidden"), pointerEvents: getComputedStyle(depth).pointerEvents,
          layer: depth.parentElement?.getAttribute("data-merman-theme-depth"), bodies: depth.children.length,
          identityFree: !depth.matches("[id], [data-id], [data-et], [name]")
            && !depth.querySelector("[id], [data-id], [data-et], [name], text, foreignObject, a, use"),
          beforeEdges: edges.every((edge) => Boolean(depth.compareDocumentPosition(edge) & Node.DOCUMENT_POSITION_FOLLOWING)),
          beforeFace: !!owner && Boolean(depth.compareDocumentPosition(owner) & Node.DOCUMENT_POSITION_FOLLOWING),
          geometry: geometry(body), faceGeometry: face ? geometry(face) : null,
          matrix: matrix(body), faceMatrix: face ? matrix(face) : [],
          inlineId: /^url\(["']?#([^)"']+)["']?\)$/u.exec(body.style.filter)?.[1], filterId: id,
          priority: body.style.getPropertyPriority("filter"),
          wins: /#([^)"']+)["']?\)$/u.exec(getComputedStyle(body).filter)?.[1] === id,
          // Neo defaults must be suppressed on both rings, without filtering the face again.
          faceUnfiltered: !!face && [face, ...face.querySelectorAll<SVGElement>("*")].every((element) =>
            !element.hasAttribute("filter") && getComputedStyle(element).filter === "none"
              && element.style.filter === "none" && element.style.getPropertyPriority("filter") === "important"),
          childrenUnfiltered: [...body.querySelectorAll("circle")].every((circle) => getComputedStyle(circle).filter === "none"),
          graphs: graphs.length,
          colorSpace: graph?.getAttribute("color-interpolation-filters"),
          primitives: [...(graph?.children ?? [])].map((element) => element.localName),
          offsets: [...(graph?.querySelectorAll("feOffset") ?? [])].map((offset) => ({
            input: offset.getAttribute("in"), result: offset.getAttribute("result"),
            dx: Number(offset.getAttribute("dx")), dy: Number(offset.getAttribute("dy")),
          })),
          matrices: [...(graph?.querySelectorAll("feColorMatrix") ?? [])].map((matrix) => ({
            input: matrix.getAttribute("in"), result: matrix.getAttribute("result"), type: matrix.getAttribute("type"),
            values: (matrix.getAttribute("values") ?? "").trim().split(/\s+/u).map(Number),
          })),
          merge: merge?.localName,
          output: [...(merge?.children ?? [])].map((node) => [node.localName, node.getAttribute("in")]),
        };
      }),
    };
  });
  expect(facts.faces.sort((first, second) => first.id!.localeCompare(second.id!))).toEqual([
    { id: "A", look: "neo", bodies: 1, tag: "rect", children: [] }, { id: "B", look: "neo", bodies: 1, tag: "rect", children: [] },
    { id: "C", look: "neo", bodies: 1, tag: "polygon", children: [] }, { id: "D", look: "neo", bodies: 1, tag: "circle", children: [] },
    { id: "E", look: "neo", bodies: 1, tag: "g", children: ["circle", "circle"] },
  ]);
  expect(facts.edges).toBe(4);
  expect(facts.depths.map(({ owner }) => owner).sort()).toEqual(["A", "B", "C", "D", "E"]);
  for (const depth of facts.depths) {
    expect(depth, `${depth.owner} owns one hard-shadow silhouette below its connections and face`).toMatchObject({
      owners: 1, look: "neo", hidden: "true", pointerEvents: "none", layer: "layer", bodies: 1,
      identityFree: true, beforeEdges: true, beforeFace: true, priority: "important", wins: true,
      faceUnfiltered: true, childrenUnfiltered: true, graphs: 1, colorSpace: "sRGB",
      primitives: ["feOffset", "feColorMatrix", "feMerge"],
      offsets: [{ input: "SourceGraphic", result: "merman-shadow-0-offset", dx: 6, dy: 6 }],
      // Zero blur is omitted; the sRGB matrix paints black while preserving the offset alpha.
      matrices: [{
        input: "merman-shadow-0-offset", result: "merman-shadow-0-shadow", type: "matrix",
        values: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
      }],
      merge: "feMerge", output: [["feMergeNode", "merman-shadow-0-shadow"]],
    });
    expect(depth.geometry, depth.owner!).toEqual(depth.faceGeometry);
    expect(depth.inlineId, depth.owner!).toBe(depth.filterId);
    for (const [index, value] of depth.matrix.entries()) expect(value, depth.owner!).toBeCloseTo(depth.faceMatrix[index], 5);
  }

  const capture = async () => {
    await root.evaluate((element) => {
      const style = element.getAttribute("style");
      element.style.setProperty("display", "none", "important");
      element.getBoundingClientRect();
      if (style === null) element.removeAttribute("style");
      else element.setAttribute("style", style);
      element.getBoundingClientRect();
    });
    return root.screenshot({ animations: "disabled" });
  };
  const original = await capture();
  const probes: { owner: string; hidden: string; restored: string }[] = [];
  const depths = root.locator("[data-merman-theme-depth='node']");
  for (const [index, fact] of facts.depths.entries()) {
    const depth = depths.nth(index);
    const style = await depth.getAttribute("style");
    let hidden: Buffer;
    try {
      await depth.evaluate((element) => (element as SVGElement).style.setProperty("visibility", "hidden", "important"));
      hidden = await capture();
    } finally {
      await depth.evaluate((element, originalStyle) => {
        if (originalStyle === null) element.removeAttribute("style");
        else element.setAttribute("style", originalStyle);
      }, style);
    }
    probes.push({ owner: fact.owner!, hidden: hidden.toString("base64"), restored: (await capture()).toString("base64") });
  }
  const contributions = await page.evaluate(async ({ original, probes }) => {
    const decode = async (encoded: string) => {
      const bytes = Uint8Array.from(atob(encoded), (character) => character.charCodeAt(0));
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/png" }));
      const canvas = document.createElement("canvas");
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext("2d")!;
      context.drawImage(bitmap, 0, 0);
      bitmap.close();
      return context.getImageData(0, 0, canvas.width, canvas.height);
    };
    const before = await decode(original);
    const changedPixels = async (encoded: string) => {
      const after = await decode(encoded);
      if (before.width !== after.width || before.height !== after.height) throw new Error("Depth probe resized the diagram");
      let count = 0;
      for (let offset = 0; offset < before.data.length; offset += 4) {
        if ([0, 1, 2, 3].some((channel) => before.data[offset + channel] !== after.data[offset + channel])) count += 1;
      }
      return count;
    };
    return Promise.all(probes.map(async (probe) => ({
      owner: probe.owner, hidden: await changedPixels(probe.hidden), restored: await changedPixels(probe.restored),
    })));
  }, { original: original.toString("base64"), probes });
  for (const probe of contributions) {
    expect(probe.hidden, `${probe.owner} depth must contribute visible shadow pixels`).toBeGreaterThan(0);
    expect(probe.restored, `${probe.owner} depth must restore its pixels`).toBe(0);
  }
}

test("a keyboard theme choice applies immediately and survives closing Appearance", async ({ page }) => {
  await start(page, "{}", null, 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  const ghibli = panel.getByRole("button", { name: /^Ghibli/ });
  await ghibli.focus();
  await ghibli.press("Enter");
  await expect(ghibli).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByTestId("theme-current-treatment")).toHaveText("Designed for this diagram");
  await expect(panel.getByTestId("theme-current-maturity")).toContainText("Alpha");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
  await expect(panel.getByTestId("theme-outcome")).toContainText("Theme applied");
  expect((await workspaceFromLink(page)).themePresetId).toBe("ghibli");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("button", { name: /^Ghibli/ })).toHaveAttribute("aria-pressed", "true");
});

test("the unified catalog switches between built-in, native and exploration themes without changing source or config", async ({ page }) => {
  const original = await start(page, '{"flowchart":{"nodeSpacing":42}}', null, "flowchart LR\nA[Keep my diagram] --> B");
  const panel = await openPanel(page);
  const choices = panel.getByTestId("theme-catalog-choices");
  await choices.locator('[data-theme-choice="preset:brutalist"]').click();
  const forest = choices.locator('[data-theme-choice="mermaid:forest"]');
  await forest.click();
  await expect(forest).toHaveAttribute("aria-pressed", "true");
  const native = await workspaceFromLink(page);
  expect(native.diagramTheme).toBe("forest");
  expect(native.themePresetId).toBeNull();
  expect(native.themeRecipeJson).toBeNull();

  const atlas = choices.locator('[data-theme-choice="exploration:atlas"]');
  await atlas.click();
  await expect(atlas).toHaveAttribute("aria-pressed", "true");
  await expect(atlas).toHaveAttribute("data-theme-source", "exploration");
  await expect(choices.locator('button[aria-pressed="true"]')).toHaveCount(1);
  await expect(panel.getByTestId("theme-current-name")).toHaveText("Atlas");
  await expect.poll(() => svg(page)).toContain('data-merman-theme-canvas="base"');
  const exploration = await workspaceFromLink(page);
  expect(exploration.themePresetId).toBeNull();
  expect(exploration.themeRecipeJson).not.toBeNull();
  expect(exploration.themeRecipeLabel).toEqual({ name: "Atlas", source: "exploration" });
  expect(exploration.diagramTheme).toBe("auto");
  expect(exploration.code).toBe(original.code);
  expect(exploration.mermaidConfig).toBe(original.mermaidConfig);
});

test("Brutalist applies hard shadows in default Neo without changing source or config, and closing preserves editor changes", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  const config = {
    layout: "dagre", htmlLabels: false,
    flowchart: { layout: "dagre", nodeSpacing: 42, curve: "linear" },
    sequence: { actorMargin: 72 }, themeVariables: { primaryColor: "#123456" },
  };
  const original = await start(page, JSON.stringify(config), null,
    "flowchart LR\nA[Authored source] --> B(Rounded) --> C{Diamond} --> D((Circle)) --> E(((Double)))");
  const panel = await openPanel(page);
  await expect(panel.getByLabel("Diagram font", { exact: true })).toHaveValue("inherit");
  await panel.getByRole("button", { name: /^Dark/ }).click();
  await panel.getByLabel("Diagram font", { exact: true }).selectOption("arial");
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  const note = panel.locator('[data-theme-application-note="flowchart-basic-shadows"]');
  await expect(note).toContainText("Classic and Neo");
  await expect(note).toContainText("basic shapes");
  const root = page.locator(".preview-container > div").first().locator("svg");
  await expect(root.locator("[data-merman-theme-depth='node']")).toHaveCount(5);
  await expect.poll(() => svg(page)).toContain("Arial");
  await page.evaluate(() => document.fonts.ready);
  await expectBrutalistDepth(page, root);
  const applied = await workspaceFromLink(page);
  expect(applied.diagramTheme).toBe("auto");
  expect(applied.diagramFont).toBe("arial");
  expect(applied.themePresetId).toBe("brutalist");
  expect(applied.code).toBe(original.code);
  expect(applied.mermaidConfig).toBe(original.mermaidConfig);

  const editedSource = "flowchart LR\nEdited[Editor changes stay] --> B";
  const editedConfig = JSON.stringify({ ...config, sequence: { actorMargin: 96 } });
  await replaceEditorSource(page, editedSource);
  await replaceMermaidConfig(page, editedConfig);
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const closed = await workspaceFromLink(page);
  expect(closed.diagramTheme).toBe("auto");
  expect(closed.diagramFont).toBe("arial");
  expect(closed.themePresetId).toBe("brutalist");
  expect(closed.code).toBe(editedSource);
  expect(closed.mermaidConfig).toBe(editedConfig);
  const reopened = await openPanel(page);
  await expect(reopened.getByLabel("Diagram font", { exact: true })).toHaveValue("arial");
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  errors.assertNone();
});

test("Undo appearance preserves source and config authored after a theme choice", async ({ page }) => {
  const original = await start(page, "{}", "brutalist", "flowchart LR\nA --> B");
  let panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Forest/ }).click();
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const editedSource = "flowchart LR\nEdited[Keep this source] --> B";
  const editedConfig = '{"flowchart":{"nodeSpacing":64},"sequence":{"actorMargin":96}}';
  await replaceEditorSource(page, editedSource);
  await replaceMermaidConfig(page, editedConfig);
  panel = await openPanel(page);
  const undo = panel.getByRole("button", { name: "Undo appearance change", exact: true });
  await expect(undo).toBeEnabled();
  await undo.click();
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  const restored = await workspaceFromLink(page);
  expect(restored.diagramTheme).toBe(original.diagramTheme);
  expect(restored.themePresetId).toBe("brutalist");
  expect(restored.code).toBe(editedSource);
  expect(restored.mermaidConfig).toBe(editedConfig);
});

test("Undo a font choice preserves authored config and subsequent source edits", async ({ page }) => {
  const config = '{"flowchart":{"look":"handDrawn","nodeSpacing":42},"sequence":{"actorMargin":72}}';
  await start(page, config, "brutalist", "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  const font = panel.getByLabel("Diagram font", { exact: true });
  await font.selectOption("arial");
  const editedSource = "flowchart LR\nEdited[Source survives undo] --> B";
  await replaceEditorSource(page, editedSource);
  await panel.getByRole("button", { name: "Undo appearance change", exact: true }).click();
  const restored = await workspaceFromLink(page);
  expect(restored.code).toBe(editedSource);
  expect(restored.mermaidConfig).toBe(config);
  expect(restored.themePresetId).toBe("brutalist");
  await expect(font).toHaveValue("inherit");
});

test("source look overrides are explained with a source editor entry", async ({ page }) => {
  const original = await start(page, '{"theme":"forest","fontFamily":"Georgia","flowchart":{"look":"neo"}}');
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  await expect(panel.getByTestId("theme-appearance-overrides")).toBeVisible();
  await openAdvanced(panel);
  const overrides = panel.getByTestId("theme-appearance-override-details");
  await expect(overrides).toContainText("forest");
  await expect(overrides).toContainText("Georgia");
  await expect(overrides).toContainText("handDrawn");
  await expect(overrides.locator("dt").filter({ hasText: "Look" })).toHaveText("Look · Source declaration");
  await overrides.getByRole("button", { name: "Edit config", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Config", exact: true })).toHaveAttribute("aria-selected", "true");
  await overrides.getByRole("button", { name: "Edit source", exact: true }).click();
  await expect(page.getByRole("tab", { name: "Code", exact: true })).toHaveAttribute("aria-selected", "true");
  const saved = await workspaceFromLink(page);
  expect(saved.code).toBe(original.code);
  expect(saved.mermaidConfig).toBe(original.mermaidConfig);
});

for (const declaration of [
  { origin: "Config", config: '{"look":"classic"}', source: "flowchart LR\nA --> B", look: "classic" },
  { origin: "Source", config: "{}", source: 'flowchart LR\nA --> B\n%%{init: {"flowchart":{"look":"neo"}}}%%', look: "neo" },
]) {
  test(`a theme choice preserves the look and its ${declaration.origin.toLowerCase()} declaration`, async ({ page }) => {
    await start(page, declaration.config, null, declaration.source);
    const panel = await openPanel(page);
    await panel.getByRole("button", { name: /^Brutalist/ }).click();
    await expect(panel.getByTestId("theme-appearance-overrides")).toBeVisible();
    await openAdvanced(panel);
    const overrides = panel.getByTestId("theme-appearance-override-details");
    await expect(overrides.locator("dt").filter({ hasText: "Look" })).toHaveText(`Look · ${declaration.origin} declaration`);
    await expect(overrides).toContainText(declaration.look);
    const saved = await workspaceFromLink(page);
    expect(saved.code).toBe(declaration.source);
    expect(saved.mermaidConfig).toBe(declaration.config);
  });
}

test("Pie keeps application notes scoped to its diagram family", async ({ page }) => {
  await start(page, "{}", "brutalist", 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Pie Chart");
  await expect(panel.locator("[data-theme-application-note]")).toHaveCount(0);
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toBeEnabled();
});

for (const scene of [
  { family: "agentflow", source: 'agentflow-beta TB\nflow support["Support"]\n  task["Resolve"]@{ shape: task }\nend' },
  { family: "usecase", source: 'usecase-beta\nactor Customer("Customer")\nsystemBoundary "Order system"\n  Checkout("Place order")\nend\nCustomer --> Checkout' },
]) {
  test(`${scene.family} applies the selected theme and preserves it in shared code`, async ({ page }) => {
    const original = await start(page, "{}", "brutalist", scene.source);
    const panel = await openPanel(page);
    await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "applied");
    await expect(panel.locator('[data-theme-reason-code="family-unadapted"]')).toHaveCount(0);
    await expect(panel.locator('[data-theme-reason-code="evidence-incomplete"]')).toHaveCount(0);
    await expect(panel.locator('[data-theme-reason-code="unsupported-paint"]')).toHaveCount(0);
    await openAdvanced(panel);
    const metadata = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
    expect(metadata.theme_execution_evidence.family_id).toBe(scene.family);
    expect(metadata.theme_execution_evidence.theme_status).toBe("verified");
    expect(metadata.theme_execution_evidence.diagnostics).toEqual([]);
    const saved = await workspaceFromLink(page);
    expect(saved.themePresetId).toBe("brutalist");
    expect(saved.code).toBe(original.code);
    expect(saved.mermaidConfig).toBe(original.mermaidConfig);
  });
}

test("invalid authored config remains repairable after an immediate theme choice and closing", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page, '{"flowchart":{"nodeSpacing":42}}', null, "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  await panel.getByRole("button", { name: /^Brutalist/ }).click();
  await page.getByRole("tab", { name: "Config", exact: true }).click();
  const config = page.getByRole("textbox", { name: "Mermaid configuration", exact: true });
  await config.focus();
  await page.keyboard.press(await page.evaluate(() => /Macintosh|Mac OS X|iPhone|iPad/u.test(navigator.userAgent) ? "Meta+A" : "Control+A"));
  await config.evaluate((element) => {
    const clipboard = new DataTransfer();
    clipboard.setData("text/plain", "{");
    element.dispatchEvent(new ClipboardEvent("paste", { bubbles: true, cancelable: true, clipboardData: clipboard }));
  });
  await expect(page.getByText(/Invalid JSON/)).toBeVisible();
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "failed");
  await panel.getByRole("button", { name: "Close appearance panel", exact: true }).click();
  const saved = await workspaceFromLink(page);
  expect(saved.mermaidConfig).toBe("{");
  expect(saved.themePresetId).toBe("brutalist");
  await replaceMermaidConfig(page, "{}");
  const reopened = await openPanel(page);
  await expect(reopened.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  await expect(reopened.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "applied");
  errors.assertNone();
});

test("hand-drawn Brutalist shows styling diagnostics in Appearance and compatibility evidence in Export", async ({ page }) => {
  await start(page, '{"flowchart":{"look":"handDrawn"}}', "brutalist", "flowchart LR\nA[Authored source] --> B");
  const panel = await openPanel(page);
  const reasons = panel.getByTestId("theme-diagnostic-reasons");
  // A multi-facet preset reports its rule's first residual, rather than one reason per facet.
  await expect(reasons.locator('[data-theme-reason-code="unsupported-paint"]')).toContainText("current look or shape");
  await expect(reasons.locator('[data-theme-reason-code="unsupported-geometry"]')).toContainText("shape or border");
  await expect(panel.locator('[data-theme-application-note="flowchart-basic-shadows"]'))
    .toContainText("Hand-drawn and other shapes may leave effects unapplied");
  await expect(panel.getByTestId("theme-target-details")).toHaveCount(0);
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await openAdvanced(panel);
  const metadata = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
  const evidence = metadata.theme_execution_evidence;
  await page.getByRole("button", { name: "Export", exact: true }).click();
  await page.getByRole("menuitem", { name: "Export image…", exact: true }).click();
  const output = page.getByRole("dialog", { name: "Export image", exact: true }).getByTestId("output-compatibility");
  await expect(output).toHaveAttribute("data-output-target-state", evidence.target_status);
  await expect(output).toHaveAttribute("data-output-evidence-version", "2");
  const details = output.locator("details").first();
  await expect(details).not.toHaveAttribute("open", "");
  await details.locator("summary").first().click();
  expect(evidence.target_reason_ids.length).toBeGreaterThan(0);
  for (const code of evidence.target_reason_ids.slice(0, 4)) {
    await expect(output.locator(`[data-output-reason-code="${code}"]`)).toBeVisible();
  }
});

test("an immediate theme choice hides previous evidence and gates artifact actions until its current render", async ({ page }) => {
  const errors = monitorBrowserErrors(page);
  await start(page, "{}", "cyberpunk");
  await page.getByRole("button", { name: "Tools", exact: true }).click();
  await page.getByRole("menuitem", { name: "Compare", exact: true }).click();
  const merman = page.locator('[data-merman-compare-engine="merman"]');
  await expect(merman).toHaveAttribute("data-merman-artifact-state", "ready");
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();

  await page.clock.install({ time: 0 });
  await page.clock.pauseAt(1000);
  const ghibli = panel.getByRole("button", { name: /^Ghibli/ });
  await ghibli.dispatchEvent("click");
  await expect(ghibli).toHaveAttribute("aria-pressed", "true");
  await expect(ghibli).toHaveAttribute("data-theme-treatment", "waiting");
  await expect(panel.getByTestId("theme-render-evidence")).toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-current-family")).toContainText("waiting for this render's detection");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("data-theme-treatment", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toHaveCount(0);
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeDisabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeDisabled();

  await page.clock.runFor(1000);
  await page.clock.resume();
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(ghibli).toHaveAttribute("data-theme-treatment", "unreviewed");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("data-theme-treatment", "dedicated");
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(panel.getByTestId("theme-execution-metadata")).toBeVisible();
  const raw = JSON.parse((await panel.getByTestId("theme-execution-metadata").textContent())!);
  expect(raw.theme_execution_evidence.family_id).toBe("flowchart");
  await expect(merman.getByRole("button", { name: "Copy SVG", exact: true })).toBeEnabled();
  await expect(merman.getByRole("button", { name: "Export image", exact: true })).toBeEnabled();
  errors.assertNone();
});

test("an invalid import leaves the current-family choices and current runtime evidence intact", async ({ page }) => {
  await start(page, "{}", "brutalist");
  const panel = await openPanel(page);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await panel.locator("summary").filter({ hasText: /^Advanced$/ }).click();
  const metadata = panel.getByTestId("theme-execution-metadata");
  await expect(metadata).toBeVisible();
  const currentMetadata = await metadata.textContent();
  const currentSvg = await svg(page);
  await panel.getByLabel("Import JSON", { exact: true }).setInputFiles({
    name: "invalid.json", mimeType: "application/json", buffer: Buffer.from("{"),
  });
  await expect(panel.getByRole("alert")).toBeVisible();
  await expect(panel.getByTestId("theme-import-candidate")).toHaveAttribute("data-theme-import-state", "invalid");
  await expect(panel.getByRole("button", { name: "Use imported theme", exact: true })).toHaveCount(0);
  await expect(panel.getByTestId("theme-render-evidence")).not.toHaveAttribute("data-theme-evidence-state", "waiting");
  await expect(metadata).toHaveText(currentMetadata!);
  expect(await svg(page)).toBe(currentSvg);
  await expect(panel.getByTestId("theme-current-family")).toHaveText("Current diagram: Flowchart");
  await expect(panel.getByRole("button", { name: /^Brutalist/ })).toHaveAttribute("aria-pressed", "true");
  expect((await workspaceFromLink(page)).themePresetId).toBe("brutalist");
});

test("Ghibli inherits its recipe font and releases explicit UI font overrides", async ({ page }) => {
  await start(page, "{}", "ghibli", 'pie title Tea time\n"Tea": 4\n"Cake": 3');
  const panel = await openPanel(page);
  const font = panel.getByRole("combobox", { name: "Diagram font", exact: true });
  await expect(font).toHaveValue("inherit");
  const titleFont = () => page.locator(".preview-container > div").first().evaluate((host) => {
    const title = host.shadowRoot?.querySelector(".pieTitleText");
    return title ? getComputedStyle(title).fontFamily : null;
  });
  await expect.poll(titleFont).toContain("Open Sans");
  await font.selectOption("georgia");
  await expect.poll(titleFont).toContain("Georgia");
  await font.selectOption("inherit");
  await expect.poll(titleFont).toContain("Open Sans");
});

test("Brutalist applies with the keyboard and Escape closes Appearance without rolling it back", async ({ page }) => {
  const original = await start(page, "{}", null, "flowchart LR\nA --> B");
  const panel = await openPanel(page);
  const brutalist = panel.getByRole("button", { name: /^Brutalist/ });
  await brutalist.focus();
  await brutalist.press("Enter");
  await expect(brutalist).toHaveAttribute("aria-pressed", "true");
  await page.keyboard.press("Escape");
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Appearance", exact: true })).toBeFocused();
  const saved = await workspaceFromLink(page);
  expect(saved.themePresetId).toBe("brutalist");
  expect(saved.code).toBe(original.code);
  expect(saved.mermaidConfig).toBe(original.mermaidConfig);
});
