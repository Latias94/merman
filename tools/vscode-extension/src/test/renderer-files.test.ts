import * as assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import Module from "node:module";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import type { RenderProcessRequest } from "../render-process.js";

describe("frozen theme recipe rendering", () => {
  for (const fail of [false, true]) {
    it(`passes frozen raw content through a private file and cleans it after ${fail ? "failure" : "success"}`, async () => {
      let capturedFile = "";
      const content = '{ "schema_version": 1, "kind": "design", "design": {} }\n';
      const renderer = loadRenderer(async (request) => {
        const index = request.invocation.args.indexOf("--theme-file");
        capturedFile = request.invocation.args[index + 1]!;
        assert.equal(await fs.readFile(capturedFile, "utf8"), content);
        assert.deepEqual(request.invocation.args.slice(-2), ["--theme-appearance", "dark"]);
        if (process.platform !== "win32") {
          assert.equal((await fs.stat(capturedFile)).mode & 0o777, 0o600);
        }
        if (fail) throw new Error("render failed");
        return { stdout: Buffer.from("<svg/>"), stderr: "", invocation: request.invocation };
      });
      const pending = renderer.renderMermanSource({
        context: { extensionUri: { fsPath: "/extension" } } as vscode.ExtensionContext,
        outputChannel: { info: () => {} } as unknown as vscode.LogOutputChannel,
        source: "flowchart LR\nA-->B",
        format: "svg",
        themeFileContent: content,
        themeAppearance: "dark",
      });
      if (fail) {
        await assert.rejects(pending, /render failed/);
      } else {
        assert.equal((await pending).stdout.toString(), "<svg/>");
      }
      assert.ok(capturedFile);
      await assert.rejects(fs.stat(capturedFile), { code: "ENOENT" });
    });
  }
});

function loadRenderer(run: (request: RenderProcessRequest) => Promise<unknown>): typeof import("../renderer.js") {
  type Loader = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const loader = Module as typeof Module & { _load: Loader };
  const original = loader._load;
  loader._load = function (request, parent, isMain) {
    if (request === "vscode") return { workspace: { isTrusted: true } };
    if (request === "./config.js") return { getCliSettings: () => ({}) };
    if (request === "./workspace.js") return { workspaceRoots: () => [] };
    if (request === "./binaries.js") {
      return {
        resolveMermanBinary: (request: { directArgs: string[] }) => ({
          command: "merman-cli", source: "test", args: request.directArgs,
        }),
      };
    }
    if (request === "./render-process.js") return { runRenderProcess: run };
    return original.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../renderer.js")];
    return require("../renderer.js") as typeof import("../renderer.js");
  } finally {
    loader._load = original;
  }
}
