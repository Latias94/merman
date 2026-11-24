import * as assert from "node:assert/strict";
import * as fs from "node:fs/promises";
import Module from "node:module";
import * as os from "node:os";
import * as path from "node:path";
import type * as vscode from "vscode";
import { describe, it } from "node:test";

import type { RenderProcessRequest } from "../render-process.js";

describe("frozen theme document rendering", () => {
  for (const fail of [false, true]) {
    const format = fail ? "svg" : "unicode";
    it(`passes frozen raw content through a private file for ${format} and cleans it up`, async () => {
      let capturedFile = "";
      const content = '{ "schema_version": 2, "extends": "studio" }\n';
      const renderer = loadRenderer(async (request) => {
        const index = request.invocation.args.indexOf("--theme");
        assert.notEqual(index, -1);
        capturedFile = request.invocation.args[index + 1]!;
        assert.equal(await fs.readFile(capturedFile, "utf8"), content);
        assert.deepEqual(request.invocation.args.slice(index + 2, index + 4), ["--appearance", "dark"]);
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
        format,
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

describe("Shoal preview layout frames", () => {
  const frameCapability = { preview_layout_frame: { protocol_version: 1, max_bytes: 128 } };
  const context = { extensionUri: { fsPath: "/extension" } } as vscode.ExtensionContext;
  const outputChannel = { info: () => {} } as unknown as vscode.LogOutputChannel;

  it("lets the CLI publish a private frame without creating a cold-preview directory", async () => {
    for (const drawShoal of [false, true]) {
      let framePath = "";
      const renderer = loadRenderer(async ({ invocation }) => {
        const { args } = invocation;
        if (args[0] === "capabilities") {
          return { stdout: Buffer.from(JSON.stringify({
            preview_layout_frame: { ...frameCapability.preview_layout_frame, private_output: true },
          })) };
        }
        assert.equal(args.includes("--preview-frame-in"), false);
        framePath = args[args.indexOf("--preview-frame-out") + 1]!;
        assert.equal(path.dirname(framePath), os.tmpdir());
        await assert.rejects(fs.stat(framePath), { code: "ENOENT" });
        if (drawShoal) await fs.writeFile(framePath, "{}", { mode: 0o600 });
        return { stdout: Buffer.from("<svg/>"), stderr: "", invocation };
      });
      const rendered = await renderer.renderMermanSource({
        context, outputChannel, source: "flowchart LR\nA-->B", format: "svg", previewLayoutFrame: {},
      });
      assert.equal(rendered.previewLayoutFrame?.toString(), drawShoal ? "{}" : undefined);
      await assert.rejects(fs.stat(framePath), { code: "ENOENT" });
    }
  });

  it("negotiates once, replays opaque bytes, and cleans up both private files", async () => {
    let probes = 0;
    let previous: Buffer | undefined;
    const frame = Buffer.from('{"family":"flowchart","nodes":{}}\n');
    const directories: string[] = [];
    const renderer = loadRenderer(async (request) => {
      const { args } = request.invocation;
      if (args[0] === "capabilities") {
        probes += 1;
        assert.equal(request.source, "");
        return { stdout: Buffer.from(JSON.stringify(frameCapability)) };
      }
      const inputIndex = args.indexOf("--preview-frame-in");
      const output = args[args.indexOf("--preview-frame-out") + 1]!;
      assert.ok(args.includes("--preview-frame-out"));
      if (previous) {
        const input = args[inputIndex + 1]!;
        assert.notEqual(input, output);
        assert.deepEqual(await fs.readFile(input), previous);
        if (process.platform !== "win32") {
          assert.equal((await fs.stat(input)).mode & 0o777, 0o600);
        }
        directories.push(input);
      } else {
        assert.equal(inputIndex, -1);
      }
      await assert.rejects(fs.stat(output), { code: "ENOENT" });
      await fs.writeFile(output, frame);
      directories.push(output);
      return { stdout: Buffer.from("<svg/>"), stderr: "", invocation: request.invocation };
    });
    for (const source of ["flowchart LR\nA-->B", "flowchart LR\nA-->C"]) {
      const rendered = await renderer.renderMermanSource({
        context, outputChannel, source, format: "svg", previewLayoutFrame: { previous },
      });
      assert.equal(rendered.stdout.toString(), "<svg/>");
      assert.deepEqual(rendered.previewLayoutFrame, frame);
      previous = rendered.previewLayoutFrame;
    }
    assert.equal(probes, 1);
    for (const filePath of directories) {
      await assert.rejects(fs.stat(filePath), { code: "ENOENT" });
    }
  });

  it("renders normally with an older CLI and never probes or uses frame flags for text and export", async () => {
    const invocations: string[][] = [];
    const renderer = loadRenderer(async (request) => {
      invocations.push(request.invocation.args);
      return {
        stdout: Buffer.from(request.invocation.args[0] === "capabilities" ? "{}" : "<svg/>"),
        stderr: "",
        invocation: request.invocation,
      };
    });
    const request = {
      context, outputChannel, source: "flowchart LR\nA-->B",
      previewLayoutFrame: { previous: Buffer.from("{}") },
    };
    await renderer.renderMermanSource({ ...request, format: "svg" });
    await renderer.renderMermanSource({ ...request, format: "ascii" });
    await renderer.renderMermanSource({ ...request, format: "svg", outputPath: "export.svg" });
    assert.equal(invocations.filter((args) => args[0] === "capabilities").length, 1);
    assert.equal(invocations.some((args) => args.some((arg) => arg.startsWith("--preview-frame-"))), false);
  });

  it("renegotiates preview frames after CLI upgrades and downgrades at the same path", async () => {
    const directory = await fs.mkdtemp(path.join(os.tmpdir(), "merman-cli-identity-"));
    try {
      const command = path.join(directory, "merman-cli");
      const replacement = path.join(directory, "replacement");
      const probes: string[] = [];
      const frameFlags: boolean[] = [];
      const renderer = loadRenderer(async ({ invocation }) => {
        const version = await fs.readFile(invocation.command, "utf8");
        const supportsFrames = version === "new";
        if (invocation.args[0] === "capabilities") {
          probes.push(version);
          return { stdout: Buffer.from(JSON.stringify(supportsFrames ? frameCapability : {})) };
        }
        const outputIndex = invocation.args.indexOf("--preview-frame-out");
        frameFlags.push(outputIndex >= 0);
        if (outputIndex >= 0) {
          if (!supportsFrames) {
            throw new Error("unexpected argument '--preview-frame-out'");
          }
          await fs.writeFile(invocation.args[outputIndex + 1]!, "{}");
        }
        return { stdout: Buffer.from("<svg/>"), stderr: "", invocation };
      }, command);
      const render = () => renderer.renderMermanSource({
        context, outputChannel, source: "flowchart LR\nA-->B", format: "svg", previewLayoutFrame: {},
      });

      await fs.writeFile(command, "old");
      await render();
      await fs.writeFile(replacement, "new");
      await fs.rename(replacement, command);
      await render();
      await fs.writeFile(command, "older");
      await render();

      assert.deepEqual(frameFlags, [false, true, false]);
      assert.deepEqual(probes, ["old", "new", "older"]);
    } finally {
      await fs.rm(directory, { recursive: true, force: true });
    }
  });

  it("cleans up after render failures and ignores missing, invalid, and oversized output frames", async () => {
    for (const failure of ["render", "missing", "invalid", "oversized"]) {
      let output = "";
      const renderer = loadRenderer(async (request) => {
        const { args } = request.invocation;
        if (args[0] === "capabilities") {
          return { stdout: Buffer.from(JSON.stringify(frameCapability)) };
        }
        output = args[args.indexOf("--preview-frame-out") + 1]!;
        if (failure !== "missing") {
          await fs.writeFile(output, failure === "oversized" ? Buffer.alloc(129, 1) : "bad");
        }
        if (failure === "render") throw new Error("render failed");
        return { stdout: Buffer.from("<svg/>"), stderr: "", invocation: request.invocation };
      });
      const render = renderer.renderMermanSource({
        context, outputChannel, source: "flowchart LR\nA-->B", format: "svg", previewLayoutFrame: {},
      });
      if (failure === "render") {
        await assert.rejects(render, /render failed/);
      } else {
        assert.equal((await render).previewLayoutFrame, undefined);
      }
      assert.ok(output);
      await assert.rejects(fs.stat(output), { code: "ENOENT" });
    }
  });
});

describe("Fit-width SVG preview", () => {
  const request = {
    context: { extensionUri: { fsPath: "/extension" } } as vscode.ExtensionContext,
    outputChannel: { info: () => {} } as unknown as vscode.LogOutputChannel,
    source: "flowchart LR\nA-->B",
    format: "svg" as const,
    previewFitWidth: 640.5,
  };

  it("negotiates fit-width with the frame in one probe and sends the CSS pixel target", async () => {
    const invocations: string[][] = [];
    const renderer = loadRenderer(async ({ invocation }) => {
      invocations.push(invocation.args);
      return {
        stdout: Buffer.from(invocation.args[0] === "capabilities"
          ? JSON.stringify({ layout_fit_width: true, preview_layout_frame: { protocol_version: 1, max_bytes: 128 } })
          : "<svg/>"),
        stderr: "",
        invocation,
      };
    });
    await renderer.renderMermanSource({ ...request, previewLayoutFrame: {} });
    assert.equal(invocations.length, 2);
    assert.deepEqual(invocations[0], ["capabilities", "--json"]);
    const args = invocations[1]!;
    const flag = args.indexOf("--fit-width");
    assert.ok(flag >= 0);
    assert.deepEqual(args.slice(flag, flag + 3), ["--fit-width", "--width", "640.5"]);
    assert.ok(args.includes("--preview-frame-out"));
  });

  it("uses the normal render for older CLIs, invalid widths, text, and exports", async () => {
    const invocations: string[][] = [];
    const renderer = loadRenderer(async ({ invocation }) => {
      invocations.push(invocation.args);
      return {
        stdout: Buffer.from(invocation.args[0] === "capabilities" ? "{}" : "<svg/>"),
        stderr: "",
        invocation,
      };
    });
    await renderer.renderMermanSource(request);
    for (const previewFitWidth of [undefined, 0, -1, NaN, Infinity]) {
      await renderer.renderMermanSource({ ...request, previewFitWidth });
    }
    await renderer.renderMermanSource({ ...request, format: "ascii" });
    await renderer.renderMermanSource({ ...request, format: "unicode" });
    await renderer.renderMermanSource({ ...request, outputPath: "export.svg" });
    await renderer.renderMermanSource({ ...request, inputKind: "svg" });
    assert.equal(invocations.filter((args) => args[0] === "capabilities").length, 1);
    assert.ok(invocations.every((args) => !args.includes("--fit-width") && !args.includes("--width")));
  });
});

function loadRenderer(
  run: (request: RenderProcessRequest) => Promise<unknown>,
  command = process.execPath,
): typeof import("../renderer.js") {
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
          command, source: "explicit", args: request.directArgs,
        }),
      };
    }
    if (request === "./render-process.js") return { runRenderProcess: run };
    return original.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../cli-capabilities.js")];
    delete require.cache[require.resolve("../renderer.js")];
    return require("../renderer.js") as typeof import("../renderer.js");
  } finally {
    loader._load = original;
  }
}
