import * as assert from "node:assert/strict";
import Module from "node:module";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import type { RenderProcessRequest } from "../render-process.js";
import { resolveThemeSelection } from "../theme-selection.js";

describe("runtime theme discovery and import", () => {
  it("uses the grouped CLI catalog's explicit Mermaid IDs and concrete preset IDs", () => {
    const { runtime } = loadRuntime();
    const discovery = runtime.parseThemeCapabilities(groupedCapabilities());
    const literals = new Set([
      ...discovery.choices.flatMap((choice) => choice.variants?.map((variant) => variant.themeId) ?? []),
      ...discovery.legacyChoices.filter((choice) => choice.origin === "mermaid").map((choice) => choice.id),
    ]);
    assert.equal(literals.size, 11);
    assert.equal(discovery.choices.filter((choice) => choice.id === "preset:editor").length, 1);
    assert.equal(discovery.choices.some((choice) => choice.id === "preset:editor-dark"), false);
    assert.equal(resolveThemeSelection(discovery, "preset:editor-dark", "follow-editor", "light").effectiveAppearance, "dark");
    assert.equal(resolveThemeSelection(discovery, "preset:editor-dark", "follow-editor", "light").themePreset, "editor-dark");
    assert.equal(resolveThemeSelection(discovery, "mermaid:redux-color", "follow-editor", "dark").theme, "redux-dark-color");
    assert.equal(discovery.choices.some((choice) => choice.id === "default"), true);
    assert.equal(discovery.choices.some((choice) => choice.id === "dark"), true);
  });

  it("degrades an old CLI to the historical source semantics without fabricating a pair", () => {
    const { runtime } = loadRuntime();
    const discovery = runtime.parseThemeCapabilities({
      cli_contract_version: 6,
      theme_presets: { schema_version: 1, presets: [] },
    });
    assert.equal(discovery.paired, false);
    assert.equal(discovery.inspect, false);
    assert.equal(discovery.choices.some((choice) => choice.id === "mermaid:neo"), false);
    assert.equal(resolveThemeSelection(discovery, "source", "follow-editor", "dark").theme, undefined);
    assert.match(discovery.notice!, /update the CLI/);
    assert.throws(() => runtime.parseThemeCapabilities({
      cli_contract_version: 7,
      theme_presets: { schema_version: 1 },
    }), /incomplete/);
  });

  it("shares discovery for an executable identity and gates import on its actual command inventory", async () => {
    const host = loadRuntime();
    const context = {} as vscode.ExtensionContext;
    const [first, second] = await Promise.all([
      host.runtime.discoverThemes(context),
      host.runtime.discoverThemes(context),
    ]);
    assert.equal(first, second);
    assert.equal(host.requests.length, 1);
    const payload = groupedCapabilities();
    payload.commands = ["render"];
    assert.equal(host.runtime.parseThemeCapabilities(payload).inspect, false);
  });

  it("sends the original file bytes to inspect and preserves the complete admitted recipe", async () => {
    const host = loadRuntime();
    host.content = '{ "schema_version": 1, "kind": "design", "design": { "appearances": {"light": {}, "dark": {}} } }\n';
    const discovery = host.runtime.parseThemeCapabilities(groupedCapabilities());
    const imported = await host.runtime.inspectThemeFile({} as vscode.ExtensionContext, fileUri(), discovery);
    assert.deepEqual(host.requests[0]?.invocation.args, ["theme", "inspect", "-"]);
    assert.equal(host.requests[0]?.source, host.content);
    assert.equal(imported.content, host.content);
    assert.deepEqual(imported.supportedAppearances, ["light", "dark"]);
    assert.equal(imported.label, "reading.theme.json");
  });

  it("forwards duplicate fields unchanged and surfaces whole-document backend rejection", async () => {
    const host = loadRuntime();
    host.content = '{"schema_version":1,"kind":"design","design":{"appearances":{"light":{},"light":{}}}}';
    host.inspectError = new Error("duplicate field at /design/appearances/light");
    await assert.rejects(
      host.runtime.inspectThemeFile({} as vscode.ExtensionContext, fileUri(), host.runtime.parseThemeCapabilities(groupedCapabilities())),
      /duplicate field/,
    );
    assert.equal(host.requests[0]?.source, host.content);
    host.inspectError = new Error("unknown reference in inactive /design/appearances/dark");
    await assert.rejects(
      host.runtime.inspectThemeFile({} as vscode.ExtensionContext, fileUri(), host.runtime.parseThemeCapabilities(groupedCapabilities())),
      /inactive/,
    );
  });

  it("bounds files before loading and does not read files without inspection capability", async () => {
    const host = loadRuntime();
    host.size = 9 * 1024 * 1024;
    await assert.rejects(
      host.runtime.inspectThemeFile({} as vscode.ExtensionContext, fileUri(), host.runtime.parseThemeCapabilities(groupedCapabilities())),
      /8 MiB/,
    );
    assert.equal(host.reads, 0);
    const unavailable = host.runtime.parseThemeCapabilities({ cli_contract_version: 6 });
    await assert.rejects(host.runtime.inspectThemeFile({} as vscode.ExtensionContext, fileUri(), unavailable), /requires a CLI/);
    assert.equal(host.reads, 0);
  });
});

function groupedCapabilities() {
  const mermaid = [
    ["default", "Default", "light"],
    ["dark", "Dark", "dark"],
    ["forest", "Forest", "light"],
    ["neutral", "Neutral", "light"],
    ["base", "Base", "light"],
  ].map(([id, name, appearance]) => ({
    id, display_name: name, default_appearance: appearance,
    appearances: [{ appearance, theme_id: id }],
  }));
  for (const [id, label, dark] of [
    ["neo", "Neo", "neo-dark"],
    ["redux", "Redux", "redux-dark"],
    ["redux-color", "Redux Color", "redux-dark-color"],
  ]) {
    mermaid.push({
      id, display_name: label, default_appearance: "light",
      appearances: [{ appearance: "light", theme_id: id }, { appearance: "dark", theme_id: dark }],
    });
  }
  return {
    cli_contract_version: 7,
    commands: ["render", "theme"],
    theme_presets: {
      schema_version: 1,
      recipe_schema_version: 1,
      authoring_request_schema_version: 1,
      authoring_result_schema_version: 1,
      mermaid_theme_designs: mermaid,
      presets: [{
        id: "editor", display_name: "Editor", available: true,
        supported_appearances: ["light", "dark"], default_appearance: "light",
        appearances: [
          { appearance: "light", concrete_preset_id: "editor-light", available: true },
          { appearance: "dark", concrete_preset_id: "editor-dark", available: true },
        ],
      }],
    },
  };
}

function fileUri(): vscode.Uri {
  return { fsPath: "/private/reading.theme.json" } as vscode.Uri;
}

function loadRuntime() {
  const host = {
    requests: [] as RenderProcessRequest[],
    content: "{}",
    size: 100,
    reads: 0,
    inspectError: undefined as Error | undefined,
  };
  type Loader = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const loader = Module as typeof Module & { _load: Loader };
  const original = loader._load;
  loader._load = function (request, parent, isMain) {
    if (request === "vscode") {
      return {
        workspace: {
          fs: {
            stat: async () => ({ size: host.size }),
            readFile: async () => { host.reads += 1; return Buffer.from(host.content); },
          },
        },
      };
    }
    if (request === "./renderer.js") {
      return { resolveCliInvocation: (_context: unknown, args: string[]) => ({ command: "merman-cli", source: "test", args }) };
    }
    if (request === "./render-process.js") {
      return {
        runRenderProcess: async (request: RenderProcessRequest) => {
          host.requests.push(request);
          if (request.invocation.args[0] === "theme" && host.inspectError) throw host.inspectError;
          const result = request.invocation.args[0] === "theme"
            ? {
              schema_version: 1,
              supported_appearances: ["light", "dark"],
              default_appearance: "light",
              appearances: [{ appearance: "light", references: [] }, { appearance: "dark", references: [] }],
            }
            : groupedCapabilities();
          return { stdout: Buffer.from(JSON.stringify(result)) };
        },
      };
    }
    return original.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../theme-runtime.js")];
    return { ...host, runtime: require("../theme-runtime.js") as typeof import("../theme-runtime.js"),
      get content() { return host.content; }, set content(value: string) { host.content = value; },
      get size() { return host.size; }, set size(value: number) { host.size = value; },
      get reads() { return host.reads; },
      get inspectError() { return host.inspectError; }, set inspectError(value: Error | undefined) { host.inspectError = value; },
    };
  } finally {
    loader._load = original;
  }
}
