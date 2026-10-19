import assert from "node:assert/strict";
import test from "node:test";
import type { ThemeDesignReferenceUse, ThemeDesignV1 } from "@mermanjs/web";
import {
  buildThemeDesignerModel,
  classifyThemeDesignDestination,
  parseThemeDesignPointer,
  searchThemeSettings,
} from "./theme-designer-model.ts";

function use(token: string, path: string): ThemeDesignReferenceUse {
  return { token, path, domain: { value_type: "color" } };
}

test("groups and search preserve arbitrary metadata and discover unnamed settings", () => {
  const design: ThemeDesignV1 = { tokens: {
    unnamed: { type: "number", value: 4 },
    accent: { type: "color", value: "red", label: "Paper ink", group: "My / theme", description: "Quiet contrast" },
    other: { type: "color", value: "red", group: "Colors" },
    empty: { type: "string", value: "normal", group: "" },
  } };
  const before = structuredClone(design);
  const model = buildThemeDesignerModel(design, []);
  assert.deepEqual(model.groups.map((group) => group.id), ["", "Colors", "My / theme", null]);
  assert.equal(model.settings.find((setting) => setting.id === "unnamed")?.label, "unnamed");
  for (const query of ["paper", "QUIET", "my / theme", "accent"])
    assert.deepEqual(searchThemeSettings(model, query).map((setting) => setting.id), ["accent"]);
  assert.deepEqual(searchThemeSettings(model, "number").map((setting) => setting.id), ["unnamed"]);
  assert.equal(searchThemeSettings(model, "  ").length, 4);
  assert.equal(searchThemeSettings(model, "unrelated").length, 0);
  assert.deepEqual(model.settings.find((setting) => setting.id === "other")?.uses, {
    direct: [], dependencies: [], downstream: [],
  });
  assert.deepEqual(design, before);
});

test("declared uses distinguish direct consumers, shared dependencies, and deduplicated downstream consumers", () => {
  const design: ThemeDesignV1 = {
    tokens: {
      cyan: { type: "color", value: "cyan" },
      alias: { type: "color", value: { ref: "cyan" } },
      glow: { type: "effect", value: { kind: "shadow-stack", composition: "independent", shadows: [
        { color: { ref: "cyan" }, offset_x: 0, offset_y: 0, std_deviation: 2 },
        { color: { ref: "alias" }, offset_x: 1, offset_y: 1, std_deviation: 3 },
      ] } },
    },
    styles: { node: { fill: { ref: "cyan" }, effect: { ref: "glow" } } },
    canvas: { base: { kind: "linear-gradient", angle_degrees: 90, stops: [
      { offset: 0, color: { ref: "alias" } }, { offset: 1, color: "black" },
    ] } },
  };
  const references = [
    use("glow", "/styles/node/effect"),
    use("cyan", "/tokens/glow/value/shadows/0/color"),
    use("cyan", "/tokens/alias/value"),
    use("alias", "/tokens/glow/value/shadows/1/color"),
    use("alias", "/canvas/base/stops/0/color"),
    use("cyan", "/styles/node/fill"),
    use("glow", "/styles/node/effect"),
  ];
  const model = buildThemeDesignerModel(design, references);
  const uses = model.settings.find((setting) => setting.id === "cyan")!.uses!;
  assert.deepEqual(uses.direct.map((row) => row.path), ["/styles/node/fill"]);
  assert.deepEqual(uses.dependencies.map((row) => row.ownerToken), ["alias", "glow"]);
  assert.deepEqual(uses.downstream.map((row) => row.path), ["/canvas/base/stops/0/color", "/styles/node/effect"]);
  assert.deepEqual(uses.downstream.find((row) => row.token === "glow")!.via, ["glow"]);
  assert.deepEqual(model, buildThemeDesignerModel(design, [...references].reverse()));
});

test("an unavailable inspection is explicit and is never reported as an empty current count", () => {
  const design: ThemeDesignV1 = { tokens: { color: { type: "color", value: "invalid" } } };
  const unavailable = buildThemeDesignerModel(design, undefined);
  assert.equal(unavailable.inspectionAvailable, false);
  assert.equal(unavailable.settings[0].uses, undefined);
  const empty = buildThemeDesignerModel(design, []);
  assert.equal(empty.inspectionAvailable, true);
  assert.deepEqual(empty.settings[0].uses, { direct: [], dependencies: [], downstream: [] });
  assert.deepEqual(buildThemeDesignerModel({}, []).settings, []);
  assert.deepEqual(buildThemeDesignerModel({}, []).groups, []);
});

test("strict JSON pointers retain escaped and prototype-like authored IDs", () => {
  assert.deepEqual(parseThemeDesignPointer(""), []);
  assert.deepEqual(parseThemeDesignPointer("/tokens/a~1b~0c/value"), ["tokens", "a/b~c", "value"]);
  assert.deepEqual(parseThemeDesignPointer("/~01/"), ["~1", ""]);
  for (const pointer of ["tokens/a", "/~", "/~2", "/a~0~x"])
    assert.equal(parseThemeDesignPointer(pointer), null);
  const design = JSON.parse('{"tokens":{"a/b~c":{"type":"color","value":"red"},"__proto__":{"type":"color","value":{"ref":"a/b~c"}},"constructor":{"type":"paint","value":{"kind":"solid","color":{"ref":"__proto__"}}}},"styles":{"__proto__":{"fill":{"ref":"constructor"}}}}') as ThemeDesignV1;
  const model = buildThemeDesignerModel(design, [
    use("a/b~c", "/tokens/__proto__/value"),
    use("__proto__", "/tokens/constructor/value/color"),
    use("constructor", "/styles/__proto__/fill"),
  ]);
  assert.equal(model.settings.length, 3);
  const escaped = model.settings.find((setting) => setting.id === "a/b~c")!;
  assert.equal(escaped.uses!.dependencies[0].ownerToken, "__proto__");
  assert.deepEqual(escaped.uses!.downstream[0].via, ["__proto__", "constructor"]);
  assert.equal(classifyThemeDesignDestination(design, "/tokens/a~1b~0c/value").kind, "token");
  const style = classifyThemeDesignDestination(design, "/styles/__proto__/fill");
  assert.equal(style.kind, "style");
  if (style.kind === "style") assert.equal(style.target, "__proto__");
});

test("destination classification preserves exact authored fields and uses JSON for unsupported or unavailable locations", () => {
  const design: ThemeDesignV1 = {
    tokens: {
      paint: { type: "paint", value: { kind: "solid", color: "red" } },
      advanced: { type: "effect", value: { kind: "graph", primitives: [] } },
    },
    styles: { node: { fill: { ref: "paint" } } },
    rules: [{ target: "actor", style: { fill: { ref: "paint" } } }],
    canvas: { layers: [{ paint: { ref: "paint" } }] },
    typography: { default: { font_size_px: 12 }, families: { sequence: { font_size_px: 14 } } },
    palettes: { series: ["red"] },
  };
  for (const [path, kind] of [
    ["/tokens/paint/value/color", "token"],
    ["/styles/node/fill", "style"],
    ["/rules/0/style/fill", "rule"],
    ["/canvas/layers/0/paint", "canvas"],
    ["/typography/default/font_size_px", "typography"],
    ["/palettes/series/0", "json"],
    ["/typography/families/sequence/font_size_px", "json"],
    ["/tokens/advanced/value/primitives", "json"],
    ["/canvas/layers/4/paint", "json"],
    ["/styles/node/fill/color", "json"],
    ["/rules/00/style/fill", "json"],
    ["/unknown", "json"],
  ] as const) {
    const destination = classifyThemeDesignDestination(design, path);
    assert.equal(destination.kind, kind, path);
    assert.equal(destination.path, path);
  }
  const rule = classifyThemeDesignDestination(design, "/rules/0/style/fill");
  assert.equal(rule.kind, "rule");
  if (rule.kind === "rule") assert.equal(rule.index, 0);
});

test("cyclic rejected drafts terminate without resolving values or inventing extra painted objects", () => {
  const design: ThemeDesignV1 = { tokens: {
    a: { type: "color", value: { ref: "b" } },
    b: { type: "color", value: { ref: "a" } },
  }, styles: { node: { fill: { kind: "solid", color: { ref: "b" } } } } };
  const before = structuredClone(design);
  const model = buildThemeDesignerModel(design, [
    use("b", "/tokens/a/value"), use("a", "/tokens/b/value"),
    use("b", "/styles/node/fill/color"),
  ]);
  const uses = model.settings.find((setting) => setting.id === "a")!.uses!;
  assert.equal(uses.dependencies.length, 1);
  assert.deepEqual(uses.downstream.map((row) => row.path), ["/styles/node/fill/color"]);
  assert.deepEqual(design, before);
});
