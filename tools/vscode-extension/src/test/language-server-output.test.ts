import * as assert from "node:assert/strict";
import Module from "node:module";
import { describe, it } from "node:test";
import type * as vscode from "vscode";

interface FakeDisposable {
  dispose(): void;
}

class FakeOutputChannel {
  disposeCalls = 0;

  constructor(
    readonly name: string,
    readonly options: unknown,
  ) {}

  appendLine(): void {}

  dispose(): void {
    this.disposeCalls += 1;
  }
}

class FakeOutputHost {
  readonly channels: FakeOutputChannel[] = [];

  readonly vscode = {
    window: {
      createOutputChannel: (name: string, options?: unknown) => {
        const channel = new FakeOutputChannel(name, options);
        this.channels.push(channel);
        return channel;
      },
    },
  };
}

describe("language server output channel lifecycle", () => {
  it("reuses one channel across language client restarts", () => {
    const host = new FakeOutputHost();
    const { ensureLanguageServerOutputChannel } =
      loadLanguageServerOutputModule(host);
    const firstContext = fakeContext();

    const first = ensureLanguageServerOutputChannel(firstContext);
    const second = ensureLanguageServerOutputChannel(firstContext);

    assert.equal(first, second);
    assert.equal(host.channels.length, 1);
    assert.equal(host.channels[0]?.name, "Merman Language Server");
    assert.deepEqual(host.channels[0]?.options, { log: true });
    assert.equal(firstContext.subscriptions.length, 1);
  });

  it("disposes the singleton with the activation context", () => {
    const host = new FakeOutputHost();
    const { ensureLanguageServerOutputChannel } =
      loadLanguageServerOutputModule(host);
    const firstContext = fakeContext();

    const first = ensureLanguageServerOutputChannel(firstContext);
    firstContext.subscriptions[0]?.dispose();
    firstContext.subscriptions[0]?.dispose();

    const secondContext = fakeContext();
    const second = ensureLanguageServerOutputChannel(secondContext);

    assert.notEqual(second, first);
    assert.equal(host.channels.length, 2);
    assert.equal(host.channels[0]?.disposeCalls, 1);
    assert.equal(host.channels[1]?.disposeCalls, 0);
    assert.equal(secondContext.subscriptions.length, 1);
  });
});

function fakeContext(): vscode.ExtensionContext & { subscriptions: FakeDisposable[] } {
  return {
    subscriptions: [],
  } as unknown as vscode.ExtensionContext & { subscriptions: FakeDisposable[] };
}

function loadLanguageServerOutputModule(
  host: FakeOutputHost,
): typeof import("../language-server-output.js") {
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
    return originalLoad.call(this, request, parent, isMain);
  };
  try {
    delete require.cache[require.resolve("../language-server-output.js")];
    return require("../language-server-output.js") as typeof import("../language-server-output.js");
  } finally {
    moduleWithLoad._load = originalLoad;
  }
}
