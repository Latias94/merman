import * as assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import Module from "node:module";
import * as os from "node:os";
import * as path from "node:path";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import { renderMermanArgs, resolvedRenderSettings } from "../render-options.js";
import { RenderProcessExitError, type RenderProcessRequest } from "../render-process.js";
import { resolveThemeSelection } from "../theme-selection.js";

const context = {} as vscode.ExtensionContext;

const mermaidThemes = [
  "default", "dark", "forest", "neutral", "base", "neo", "neo-dark",
  "redux", "redux-dark", "redux-color", "redux-dark-color",
];

function v2Catalog() {
  return {
    schema_version: 2,
    presets: [
      { id: "studio", name: "Studio", description: "Reading", category: "reading",
        appearances: ["light", "dark"], default_appearance: "light", swatches: [] },
      { id: "paper-cut", name: "Paper Cut", description: "Layered paper", category: "art",
        appearances: ["light"], default_appearance: "light", swatches: [] },
    ],
    mermaid_themes: mermaidThemes,
    families: [{ id: "flowchart", capabilities: 255 }],
  };
}

describe("runtime theme discovery and import", () => {
  it("uses the v2 preset IDs and literal Mermaid Classic IDs", () => {
    const { runtime } = loadRuntime();
    const discovery = runtime.parseThemeCatalog(v2Catalog());
    assert.deepEqual(discovery.choices.filter((choice) => choice.origin === "mermaid").map((choice) => choice.id), mermaidThemes);
    assert.deepEqual(discovery.choices.filter((choice) => choice.origin === "preset").map((choice) => choice.id), ["studio", "paper-cut"]);
    assert.deepEqual(discovery.choices.find((choice) => choice.id === "file")?.label, "Custom File");
    assert.equal(resolveThemeSelection(discovery, "paper-cut", "follow-editor", "dark").effectiveAppearance, "light");
    assert.equal(resolveThemeSelection(discovery, "redux-dark-color", "follow-editor", "light").theme, "redux-dark-color");
  });

  it("leaves Source available without interpreting an unsupported theme catalog", async () => {
    const { runtime } = loadRuntime(process.execPath, () => ({ schema_version: 1, presets: [] }));
    const discovery = await runtime.discoverThemes(context);
    assert.deepEqual(discovery.choices.map((choice) => choice.id), ["source"]);
    assert.equal(resolveThemeSelection(discovery, "source", "follow-editor", "dark").theme, undefined);
    assert.match(discovery.notice!, /update the CLI/i);
    assert.throws(() => runtime.parseThemeCatalog({ schema_version: 2 }), /incomplete/);
  });

  it("uses the older CLI's literal Classic IDs and compatible render arguments", async () => {
    const host = loadRuntime(process.execPath, () => {
      throw new RenderProcessExitError("unrecognized subcommand 'list'", Buffer.alloc(0));
    });
    host.capabilities = {
      cli_contract_version: 8,
      theme_presets: {
        schema_version: 1,
        presets: [{ id: "editor" }],
        mermaid_theme_designs: [
          { id: "forest", appearances: [{ appearance: "light", theme_id: "forest" }] },
          { id: "neo", appearances: [{ appearance: "light", theme_id: "neo" }, { appearance: "dark", theme_id: "neo-dark" }] },
        ],
      },
    };

    const discovery = await host.runtime.discoverThemes(context);
    assert.deepEqual(discovery.choices.map((choice) => choice.id), ["source", "forest", "neo", "neo-dark"]);
    assert.deepEqual(host.requests.map(({ invocation }) => invocation.args), [
      ["theme", "list", "--json"], ["capabilities", "--json"],
    ]);
    assert.match(discovery.notice!, /update the CLI/i);
    const forest = resolveThemeSelection(discovery, "forest", "follow-editor", "dark");
    assert.deepEqual(renderMermanArgs({ ...resolvedRenderSettings(forest), format: "svg" }).slice(7), ["--theme", "forest"]);
  });

  it("queries theme list once per executable identity", async () => {
    const host = loadRuntime();
    const [first, second] = await Promise.all([host.runtime.discoverThemes(context), host.runtime.discoverThemes(context)]);
    assert.equal(first, second);
    assert.equal(host.requests.length, 1);
    assert.deepEqual(host.requests[0]?.invocation.args, ["theme", "list", "--json"]);
  });

  it("refreshes the theme catalog when the CLI changes at the same path", async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "merman-theme-cli-identity-"));
    try {
      const command = path.join(directory, "merman-cli");
      const host = loadRuntime(command, async () =>
        await fs.readFile(command, "utf8") === "current" ? v2Catalog() : {},
      );
      await fs.writeFile(command, "old");
      assert.equal((await host.runtime.discoverThemes(context)).choices.length, 1);
      await fs.writeFile(command, "current");
      assert.ok((await host.runtime.discoverThemes(context)).choices.some((choice) => choice.id === "studio"));
      await fs.writeFile(command, "old");
      assert.equal((await host.runtime.discoverThemes(context)).choices.length, 1);
      assert.equal(host.requests.filter(({ invocation }) => invocation.args[0] === "theme").length, 3);
    } finally {
      await fs.rm(directory, { recursive: true, force: true });
    }
  });

  it("sends the original document to check and preserves inherited and authored appearances", async () => {
    const host = loadRuntime();
    const discovery = host.runtime.parseThemeCatalog(v2Catalog());
    host.content = '{ "schema_version": 2, "extends": "studio", "default_appearance": "dark" }\n';
    host.checkAppearance = "dark";
    const imported = await host.runtime.checkThemeFile(context, fileUri(), discovery);
    assert.deepEqual(host.requests[0]?.invocation.args, ["theme", "check", "-", "--json"]);
    assert.equal(host.requests[0]?.source, host.content);
    assert.equal(imported.content, host.content);
    assert.deepEqual(imported.supportedAppearances, ["light", "dark"]);
    assert.equal(imported.defaultAppearance, "dark");
    assert.equal(imported.label, "reading.theme.json");

    host.content = '{ "schema_version": 2, "appearances": { "dark": {} } }';
    const single = await host.runtime.checkThemeFile(context, fileUri(), discovery);
    assert.deepEqual(single.supportedAppearances, ["dark"]);
    assert.equal(resolveThemeSelection(discovery, "file", "follow-editor", "light", single).effectiveAppearance, "dark");
  });

  it("checks a dual-appearance file without a default using the selected appearance", async () => {
    const host = loadRuntime();
    const discovery = host.runtime.parseThemeCatalog(v2Catalog());
    host.content = `{
  "schema_version": 2,
  "appearances": {
    "light": { "seed": { "canvas": "#FFFFFF", "ink": "#111111", "accent": "#0052CC" } },
    "dark": { "seed": { "canvas": "#111111", "ink": "#FFFFFF", "accent": "#2288FF" } }
  }
}\n`;
    host.check = ({ invocation }) => {
      const index = invocation.args.indexOf("--appearance");
      if (index < 0) {
        throw new RenderProcessExitError("theme check exited with status 2", Buffer.from(JSON.stringify({
          ok: false, path: "default_appearance", message: "select an appearance or declare a default",
        })));
      }
      return invocation.args[index + 1]!;
    };
    for (const [preference, expected] of [["follow-editor", "light"], ["dark", "dark"]] as const) {
      const imported = await host.runtime.checkThemeFile(context, fileUri(), discovery, preference);
      const request = host.requests.at(-1)!;
      assert.deepEqual(request.invocation.args, ["theme", "check", "-", "--json", "--appearance", expected]);
      assert.equal(request.source, host.content);
      assert.equal(imported.content, host.content);
      assert.deepEqual(imported.supportedAppearances, ["light", "dark"]);
      assert.equal(resolveThemeSelection(discovery, "file", preference, "light", imported).effectiveAppearance, expected);
    }
  });

  it("forwards duplicate fields unchanged and surfaces the CLI's JSON diagnostic on failure", async () => {
    const host = loadRuntime();
    host.content = '{"schema_version":2,"appearances":{"light":{},"light":{}}}';
    host.checkError = new RenderProcessExitError("merman-cli exited with status 2", Buffer.from(JSON.stringify({
      ok: false, message: "duplicate field", path: "appearances.light", line: 1, column: 48,
    })));
    await assert.rejects(
      host.runtime.checkThemeFile(context, fileUri(), host.runtime.parseThemeCatalog(v2Catalog())),
      /duplicate field \(appearances\.light, line 1, column 48\)/,
    );
    assert.equal(host.requests[0]?.source, host.content);
  });

  it("bounds files before loading and only reads them with theme check support", async () => {
    const host = loadRuntime();
    host.size = 9 * 1024 * 1024;
    await assert.rejects(
      host.runtime.checkThemeFile(context, fileUri(), host.runtime.parseThemeCatalog(v2Catalog())),
      /8 MiB/,
    );
    assert.equal(host.reads, 0);
    await assert.rejects(host.runtime.checkThemeFile(context, fileUri(), { choices: [] }), /require a CLI/);
    assert.equal(host.reads, 0);
  });
});

function fileUri(): vscode.Uri {
  return { fsPath: "/private/reading.theme.json" } as vscode.Uri;
}

function loadRuntime(command = process.execPath, catalog: () => unknown = v2Catalog) {
  const host = {
    requests: [] as RenderProcessRequest[],
    content: "{}",
    size: 100,
    reads: 0,
    capabilities: {} as unknown,
    check: undefined as ((request: RenderProcessRequest) => string) | undefined,
    checkAppearance: "light",
    checkError: undefined as Error | undefined,
  };
  type Loader = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const loader = Module as typeof Module & { _load: Loader };
  const original = loader._load;
  loader._load = function (request, parent, isMain) {
    if (request === "vscode") {
      return {
        window: { activeColorTheme: { kind: 1 } },
        workspace: {
          fs: {
            stat: async () => ({ size: host.size }),
            readFile: async () => { host.reads += 1; return Buffer.from(host.content); },
          },
        },
      };
    }
    if (request === "./renderer.js") {
      return { resolveCliInvocation: (_context: unknown, args: string[]) => ({ command, source: "explicit", args }) };
    }
    if (request === "./render-process.js") {
      return {
        RenderProcessExitError,
        runRenderProcess: async (request: RenderProcessRequest) => {
          host.requests.push(request);
          const checking = request.invocation.args[1] === "check";
          if (checking && host.checkError) throw host.checkError;
          const result = checking
            ? { ok: true, resolved: { appearance: host.check?.(request) ?? host.checkAppearance }, notices: [], contrast: [] }
            : request.invocation.args[0] === "capabilities" ? host.capabilities : await catalog();
          return { stdout: Buffer.from(JSON.stringify(result)) };
        },
      };
    }
    return original.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../cli-capabilities.js")];
    delete require.cache[require.resolve("../theme-runtime.js")];
    return Object.assign(host, { runtime: require("../theme-runtime.js") as typeof import("../theme-runtime.js") });
  } finally {
    loader._load = original;
  }
}
