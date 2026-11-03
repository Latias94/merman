import * as assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import Module from "node:module";
import * as path from "node:path";
import { describe, it } from "node:test";
import { transformSync } from "esbuild";
import type * as vscode from "vscode";

import { BUNDLED_ANALYSIS_CONFIG_PROJECTION } from "../generated/analysis-config-baseline.js";

const states = { Stopped: 1, Starting: 2, Running: 3, StartFailed: 4 };

class FakeLanguageClient {
  state = states.Stopped;
  stopped = false;
  readonly notifications: unknown[] = [];
  readonly initializeResult = {
    capabilities: {
      experimental: { merman: { requests: { configSchema: "merman/configSchema" } } },
    },
  };
  private readonly stateListeners: Array<(event: { newState: number; oldState: number }) => void> = [];

  constructor(
    private readonly host: FakeExtensionHost,
    readonly serverOptions: { run: { command: string; args: string[] } },
    readonly outputChannel: { appendLine(): void; warn(): void },
  ) {}

  async setTrace(): Promise<void> {}

  onDidChangeState(listener: (event: { newState: number; oldState: number }) => void): void {
    this.stateListeners.push(listener);
  }

  async start(): Promise<void> {
    this.changeState(states.Running);
  }

  async stop(): Promise<void> {
    this.stopped = true;
    this.changeState(states.Stopped);
  }

  async sendRequest(): Promise<unknown> {
    return {
      ...BUNDLED_ANALYSIS_CONFIG_PROJECTION,
      version: 2,
      rule_catalog_method: "merman/ruleCatalog",
    };
  }

  async sendNotification(_method: string, payload: unknown): Promise<void> {
    this.notifications.push(payload);
    const gate = this.host.configurationGate;
    this.host.configurationGate = undefined;
    if (gate) {
      gate.entered.resolve();
      await gate.release.promise;
    }
  }

  private changeState(newState: number): void {
    const oldState = this.state;
    this.state = newState;
    for (const listener of this.stateListeners) {
      listener({ newState, oldState });
    }
  }
}

class FakeExtensionHost {
  readonly clients: FakeLanguageClient[] = [];
  readonly errors: string[] = [];
  readonly status = { text: "", tooltip: "", show() {}, dispose() {} };
  readonly settings = new Map<string, unknown>([
    ["merman.server.path", `${path.dirname(process.execPath)}/./${path.basename(process.execPath)}`],
  ]);
  readonly context = {
    extensionUri: { fsPath: process.cwd() },
    subscriptions: [],
  } as unknown as vscode.ExtensionContext;
  configurationGate: {
    entered: ReturnType<typeof deferred>;
    release: ReturnType<typeof deferred>;
  } | undefined;
  private configurationListener: ((event: vscode.ConfigurationChangeEvent) => Promise<void>) | undefined;

  readonly vscode = {
    StatusBarAlignment: { Left: 1 },
    env: {},
    commands: {
      executeCommand: async () => {},
      registerCommand: () => ({ dispose() {} }),
    },
    window: {
      createStatusBarItem: () => this.status,
      createOutputChannel: () => ({ appendLine() {}, warn() {}, dispose() {} }),
      showErrorMessage: (message: string) => this.errors.push(message),
    },
    workspace: {
      isTrusted: true,
      getConfiguration: (section: string) => ({
        get: (setting: string, fallback?: unknown) =>
          this.settings.get(`${section}.${setting}`) ?? fallback,
      }),
      onDidChangeConfiguration: (listener: (event: vscode.ConfigurationChangeEvent) => Promise<void>) => {
        this.configurationListener = listener;
        return { dispose() {} };
      },
      onDidGrantWorkspaceTrust: () => ({ dispose() {} }),
    },
  };

  changeSetting(setting: string, value: unknown): Promise<void> {
    this.settings.set(setting, value);
    assert.ok(this.configurationListener);
    return this.configurationListener({
      affectsConfiguration: (section: string) =>
        setting === section || setting.startsWith(`${section}.`),
    });
  }

  blockNextConfiguration() {
    const gate = {
      entered: deferred(),
      release: deferred(),
    };
    this.configurationGate = gate;
    return gate;
  }
}

if (process.env.MERMAN_LIFECYCLE_EXTENSION_HOST !== "1") {
  describe("extension configuration lifecycle", () => {
    it("keeps a required restart when a preview setting changes behind a blocked notification", async () => {
      const host = new FakeExtensionHost();
      const extension = loadExtension(host);
      await extension.activate(host.context);
      const originalClient = host.clients[0];
      assert.ok(originalClient);
      const gate = host.blockNextConfiguration();
      const blocked = host.changeSetting("merman.analysis.lint.profile", "strict");
      await gate.entered.promise;

      try {
        const restart = host.changeSetting("merman.server.path", process.execPath);
        const preview = host.changeSetting("merman.preview.background", "dark");
        gate.release.resolve();
        await Promise.all([blocked, restart, preview]);

        assert.equal(originalClient.stopped, true);
        assert.equal(host.clients.length, 2);
        assert.equal(host.clients[1]?.serverOptions.run.command, process.execPath);
        assert.deepEqual(host.clients[1]?.notifications.at(-1), {
          settings: { analysis: { lint: { profile: "strict" } } },
        });
        assert.match(host.status.text, /Running$/);
        assert.deepEqual(host.errors, []);
      } finally {
        gate.release.resolve();
        await extension.deactivate();
      }
    });

    it("stops the client before showing disabled status after a later preview setting event", async () => {
      const host = new FakeExtensionHost();
      const extension = loadExtension(host);
      await extension.activate(host.context);
      const originalClient = host.clients[0];
      assert.ok(originalClient);
      const gate = host.blockNextConfiguration();
      const blocked = host.changeSetting("merman.analysis.lint.profile", "strict");
      await gate.entered.promise;

      try {
        const disable = host.changeSetting("merman.languageIntelligence.enabled", false);
        const preview = host.changeSetting("merman.preview.background", "dark");
        gate.release.resolve();
        await Promise.all([blocked, disable, preview]);

        assert.equal(originalClient.stopped, true);
        assert.equal(host.clients.length, 1);
        assert.match(host.status.text, /Disabled$/);
        assert.deepEqual(host.errors, []);
      } finally {
        gate.release.resolve();
        await extension.deactivate();
      }
    });

    it("pushes the latest analysis configuration after an unrelated queued setting event", async () => {
      const host = new FakeExtensionHost();
      const extension = loadExtension(host);
      await extension.activate(host.context);
      const activeClient = host.clients[0];
      assert.ok(activeClient);
      const gate = host.blockNextConfiguration();
      const blocked = host.changeSetting("merman.analysis.lint.profile", "core");
      await gate.entered.promise;

      try {
        const analysis = host.changeSetting("merman.analysis.lint.profile", "strict");
        const preview = host.changeSetting("merman.preview.background", "dark");
        gate.release.resolve();
        await Promise.all([blocked, analysis, preview]);

        assert.equal(host.clients.length, 1);
        assert.equal(activeClient.stopped, false);
        assert.deepEqual(activeClient.notifications.at(-1), {
          settings: { analysis: { lint: { profile: "strict" } } },
        });
        assert.match(host.status.text, /Running$/);
      } finally {
        gate.release.resolve();
        await extension.deactivate();
      }
    });

    it("cleans up stale startup and starts with the latest settings after a configuration event", async () => {
      const host = new FakeExtensionHost();
      const extension = loadExtension(host);
      const gate = host.blockNextConfiguration();
      const activating = extension.activate(host.context);
      await gate.entered.promise;

      try {
        const analysis = host.changeSetting("merman.analysis.lint.profile", "strict");
        const preview = host.changeSetting("merman.preview.background", "dark");
        gate.release.resolve();
        await Promise.all([activating, analysis, preview]);

        assert.equal(host.clients.length, 2);
        assert.equal(host.clients[0]?.stopped, true);
        assert.equal(host.clients[1]?.stopped, false);
        assert.deepEqual(host.clients[1]?.notifications.at(-1), {
          settings: { analysis: { lint: { profile: "strict" } } },
        });
        assert.match(host.status.text, /Running$/);
        assert.deepEqual(host.errors, []);
      } finally {
        gate.release.resolve();
        await extension.deactivate();
      }
    });
  });
}

export async function run(): Promise<void> {
  assert.equal(process.env.MERMAN_LIFECYCLE_EXTENSION_HOST, "1");
  const vscodeHost = require("vscode") as typeof vscode;
  const failures: unknown[] = [];
  for (const scenario of ["restart", "disable"] as const) {
    const host = new FakeExtensionHost();
    const configuration = vscodeHost.workspace.getConfiguration("merman");
    await configuration.update("server.path", host.settings.get("merman.server.path"), 1);
    await configuration.update("languageIntelligence.enabled", true, 1);
    await configuration.update("analysis.lint.profile", "core", 1);
    await configuration.update("preview.background", "paper", 1);
    const completions: Promise<void>[] = [];
    let received: ReturnType<typeof deferred> | undefined;
    host.vscode.workspace.getConfiguration = (section) => vscodeHost.workspace.getConfiguration(section);
    host.vscode.workspace.onDidChangeConfiguration = (listener) =>
      vscodeHost.workspace.onDidChangeConfiguration((event) => {
        if (event.affectsConfiguration("merman")) {
          completions.push(listener(event));
          received?.resolve();
        }
      });
    const extension = loadExtension(host);
    await extension.activate(host.context);
    const originalClient = host.clients[0];
    assert.ok(originalClient);
    const gate = host.blockNextConfiguration();
    const changeSetting = async (setting: string, value: unknown): Promise<void> => {
      received = deferred();
      let timeout: NodeJS.Timeout | undefined;
      try {
        await configuration.update(setting, value, 1);
        await Promise.race([
          received.promise,
          new Promise<never>((_resolve, reject) => {
            timeout = setTimeout(() => reject(new Error(`No VS Code setting event for ${setting}`)), 10000);
          }),
        ]);
      } finally {
        clearTimeout(timeout);
      }
    };

    try {
      await changeSetting("analysis.lint.profile", "strict");
      await gate.entered.promise;
      if (scenario === "restart") {
        await changeSetting("server.path", process.execPath);
      } else {
        await changeSetting("languageIntelligence.enabled", false);
      }
      await changeSetting("preview.background", "dark");
      gate.release.resolve();
      await Promise.all(completions);

      assert.equal(originalClient.stopped, true, `${scenario} must stop the original client`);
      assert.deepEqual(host.errors, []);
      if (scenario === "restart") {
        assert.equal(host.clients.length, 2);
        assert.equal(host.clients[1]?.serverOptions.run.command, process.execPath);
        const notification = host.clients[1]?.notifications.at(-1) as {
          settings: { analysis: { lint: { profile: string } } };
        };
        assert.equal(notification.settings.analysis.lint.profile, "strict");
        assert.match(host.status.text, /Running$/);
      } else {
        assert.equal(host.clients.length, 1);
        assert.match(host.status.text, /Disabled$/);
      }
      console.log(`Passed real VS Code setting events: ${scenario} -> preview.background`);
    } catch (error) {
      console.error(error);
      failures.push(error);
    } finally {
      gate.release.resolve();
      await extension.deactivate();
      for (const subscription of host.context.subscriptions) {
        subscription.dispose();
      }
    }
  }
  assert.equal(failures.length, 0, "real VS Code setting lifecycle scenarios must pass");
}

function deferred(): { promise: Promise<void>; resolve(): void } {
  let resolve!: () => void;
  const promise = new Promise<void>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

function loadExtension(host: FakeExtensionHost): typeof import("../extension.js") {
  const extensionPath = require.resolve("../extension.js");
  const extensionModule = new Module(extensionPath, module) as Module & {
    _compile(source: string, filename: string): void;
  };
  extensionModule.filename = extensionPath;
  extensionModule.paths = module.paths;
  const { code } = transformSync(
    readFileSync(path.resolve(__dirname, "../../src/extension.ts"), "utf8"),
    { loader: "ts", format: "cjs", target: "node22" },
  );
  type LoadModule = (this: unknown, request: string, parent: unknown, isMain: boolean) => unknown;
  const moduleWithLoad = Module as typeof Module & { _load: LoadModule };
  const originalLoad = moduleWithLoad._load;
  moduleWithLoad._load = function patchedLoad(
    this: unknown,
    request: string,
    parent: unknown,
    isMain: boolean,
  ): unknown {
    if (request === "vscode") {
      return host.vscode;
    }
    if (request === "vscode-languageclient/node") {
      return {
        State: states,
        Trace: { Off: 0 },
        TransportKind: { stdio: 0 },
        RevealOutputChannelOn: { Never: 0 },
        LanguageClient: class extends FakeLanguageClient {
          constructor(
            _id: string,
            _name: string,
            serverOptions: ConstructorParameters<typeof FakeLanguageClient>[1],
            options: { outputChannel: FakeLanguageClient["outputChannel"] },
          ) {
            super(host, serverOptions, options.outputChannel);
            host.clients.push(this);
          }
        },
      };
    }
    if (request === "./preview.js") {
      return { registerPreview() {} };
    }
    if (request === "./export.js") {
      return { registerExport() {} };
    }
    if (request === "./codelens.js") {
      return { registerSourceCodeLens() {} };
    }
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    for (const name of ["server", "config", "workspace", "language-server-output"]) {
      delete require.cache[require.resolve(`../${name}.js`)];
    }
    extensionModule._compile(code, extensionPath);
    return extensionModule.exports as typeof import("../extension.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}
