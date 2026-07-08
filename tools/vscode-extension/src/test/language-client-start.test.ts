import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { startLanguageClientWithCleanup } from "../language-client-start.js";

class FakeLanguageClient {
  startCalls = 0;
  stopCalls = 0;

  constructor(private readonly startError?: Error) {}

  async start(): Promise<void> {
    this.startCalls += 1;
    if (this.startError) {
      throw this.startError;
    }
  }

  async stop(): Promise<void> {
    this.stopCalls += 1;
  }
}

describe("language client startup cleanup", () => {
  it("assigns the client only after start and configuration push succeed", async () => {
    const client = new FakeLanguageClient();
    let assigned: FakeLanguageClient | undefined;
    let pushCalls = 0;

    await startLanguageClientWithCleanup({
      client,
      generation: 1,
      startingTooltip: "Starting language server",
      failedTooltip: "Merman language server failed to start.",
      isCurrentGeneration: () => true,
      wireClient: () => {},
      updateStatus: () => {},
      pushConfiguration: async () => {
        pushCalls += 1;
      },
      assignClient: (activeClient) => {
        assigned = activeClient;
      },
      clearClientIfCurrent: () => {},
      showStartError: () => {},
    });

    assert.equal(client.startCalls, 1);
    assert.equal(pushCalls, 1);
    assert.equal(assigned, client);
  });

  it("clears and stops a client whose start rejects", async () => {
    const client = new FakeLanguageClient(new Error("server start exploded"));
    let assigned: FakeLanguageClient | undefined = client;
    let clearCalls = 0;
    const errors: string[] = [];

    await assert.rejects(
      () =>
        startLanguageClientWithCleanup({
          client,
          generation: 1,
          startingTooltip: "Starting language server",
          failedTooltip: "Merman language server failed to start.",
          isCurrentGeneration: () => true,
          wireClient: () => {},
          updateStatus: () => {},
          pushConfiguration: async () => {},
          assignClient: (activeClient) => {
            assigned = activeClient;
          },
          clearClientIfCurrent: (activeClient) => {
            if (assigned === activeClient) {
              assigned = undefined;
            }
            clearCalls += 1;
          },
          showStartError: (message) => {
            errors.push(message);
          },
        }),
      /server start exploded/,
    );

    assert.equal(client.startCalls, 1);
    assert.equal(client.stopCalls, 1);
    assert.equal(clearCalls, 1);
    assert.equal(assigned, undefined);
    assert.deepEqual(errors, [
      "Merman language server failed to start: server start exploded",
    ]);
  });

  it("clears and stops a client whose configuration push rejects", async () => {
    const client = new FakeLanguageClient();
    let assigned: FakeLanguageClient | undefined = client;
    let clearCalls = 0;
    const errors: string[] = [];

    await assert.rejects(
      () =>
        startLanguageClientWithCleanup({
          client,
          generation: 1,
          startingTooltip: "Starting language server",
          failedTooltip: "Merman language server failed to start.",
          isCurrentGeneration: () => true,
          wireClient: () => {},
          updateStatus: () => {},
          pushConfiguration: async () => {
            throw new Error("configuration push exploded");
          },
          assignClient: (activeClient) => {
            assigned = activeClient;
          },
          clearClientIfCurrent: (activeClient) => {
            if (assigned === activeClient) {
              assigned = undefined;
            }
            clearCalls += 1;
          },
          showStartError: (message) => {
            errors.push(message);
          },
        }),
      /configuration push exploded/,
    );

    assert.equal(client.startCalls, 1);
    assert.equal(client.stopCalls, 1);
    assert.equal(clearCalls, 1);
    assert.equal(assigned, undefined);
    assert.deepEqual(errors, [
      "Merman language server failed to start: configuration push exploded",
    ]);
  });

  it("stops without assigning when the lifecycle generation changes during startup", async () => {
    const client = new FakeLanguageClient();
    let assigned = false;

    await startLanguageClientWithCleanup({
      client,
      generation: 1,
      startingTooltip: "Starting language server",
      failedTooltip: "Merman language server failed to start.",
      isCurrentGeneration: () => false,
      wireClient: () => {},
      updateStatus: () => {},
      pushConfiguration: async () => {
        throw new Error("must not push stale configuration");
      },
      assignClient: () => {
        assigned = true;
      },
      clearClientIfCurrent: () => {},
      showStartError: () => {},
    });

    assert.equal(client.startCalls, 1);
    assert.equal(client.stopCalls, 1);
    assert.equal(assigned, false);
  });

  it("reports stale startup after configuration push invalidates the lifecycle generation", async () => {
    const client = new FakeLanguageClient();
    let isCurrent = true;
    let assigned = false;
    let staleStartups = 0;

    await startLanguageClientWithCleanup({
      client,
      generation: 1,
      startingTooltip: "Starting language server",
      failedTooltip: "Merman language server failed to start.",
      isCurrentGeneration: () => isCurrent,
      wireClient: () => {},
      updateStatus: () => {},
      pushConfiguration: async () => {
        isCurrent = false;
      },
      assignClient: () => {
        assigned = true;
      },
      clearClientIfCurrent: () => {},
      showStartError: () => {},
      onStaleStartup: () => {
        staleStartups += 1;
      },
    });

    assert.equal(client.startCalls, 1);
    assert.equal(client.stopCalls, 1);
    assert.equal(assigned, false);
    assert.equal(staleStartups, 1);
  });
});
