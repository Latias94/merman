import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { runServerBackedCommand } from "../server-backed-command.js";
import { settleWithin } from "./test-utils.js";

describe("server-backed commands", () => {
  it("warns without awaiting notification when language intelligence is disabled", async () => {
    let requestCalls = 0;
    let handleCalls = 0;
    const warnings: string[] = [];

    const outcome = await settleWithin(
      runServerBackedCommand({
        settings: { enabled: false },
        client: {},
        request: async () => {
          requestCalls += 1;
          return {};
        },
        handleResponse: async () => {
          handleCalls += 1;
        },
        failureMessagePrefix: "Merman test request failed",
        showWarningMessage: (message) => {
          warnings.push(message);
          return new Promise(() => {});
        },
      }),
      "server-backed command notification should not be awaited",
    );

    assert.equal(outcome, "disabled");
    assert.equal(requestCalls, 0);
    assert.equal(handleCalls, 0);
    assert.match(warnings[0] ?? "", /language intelligence is disabled/);
  });

  it("warns without requesting when the language server is missing", async () => {
    let requestCalls = 0;
    const warnings: string[] = [];

    const outcome = await runServerBackedCommand({
      settings: { enabled: true },
      client: undefined,
      request: async () => {
        requestCalls += 1;
        return {};
      },
      handleResponse: async () => {},
      failureMessagePrefix: "Merman test request failed",
      showWarningMessage: (message) => {
        warnings.push(message);
      },
    });

    assert.equal(outcome, "missingClient");
    assert.equal(requestCalls, 0);
    assert.deepEqual(warnings, ["Merman language server is not running."]);
  });

  it("shows a warning and skips response handling when the LSP request rejects", async () => {
    let handleCalls = 0;
    const warnings: string[] = [];

    const outcome = await runServerBackedCommand({
      settings: { enabled: true },
      client: {},
      request: async () => {
        throw new Error("server exploded");
      },
      handleResponse: async () => {
        handleCalls += 1;
      },
      failureMessagePrefix: "Merman config schema request failed",
      showWarningMessage: (message) => {
        warnings.push(message);
      },
    });

    assert.equal(outcome, "failed");
    assert.equal(handleCalls, 0);
    assert.deepEqual(warnings, ["Merman config schema request failed: server exploded"]);
  });

  it("handles successful responses without warning", async () => {
    const warnings: string[] = [];
    const handled: string[] = [];

    const outcome = await runServerBackedCommand({
      settings: { enabled: true },
      client: { value: "catalog" },
      request: async (client) => `${client.value}-response`,
      handleResponse: async (response) => {
        handled.push(response);
      },
      failureMessagePrefix: "Merman rule catalog request failed",
      showWarningMessage: (message) => {
        warnings.push(message);
      },
    });

    assert.equal(outcome, "completed");
    assert.deepEqual(handled, ["catalog-response"]);
    assert.deepEqual(warnings, []);
  });
});
