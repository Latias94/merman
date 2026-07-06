import * as assert from "node:assert/strict";
import { describe, it } from "node:test";

import { runRestartLanguageServerCommand } from "../restart-command.js";
import { settleWithin } from "./test-utils.js";

describe("restart language server command", () => {
  it("warns without restarting when language intelligence is disabled", async () => {
    let disabledStatusUpdates = 0;
    let restartCalls = 0;
    const warnings: string[] = [];
    const infos: string[] = [];

    const outcome = await settleWithin(
      runRestartLanguageServerCommand({
        settings: { enabled: false },
        updateDisabledStatus: () => {
          disabledStatusUpdates += 1;
        },
        runRestart: async () => {
          restartCalls += 1;
        },
        showWarningMessage: (message) => {
          warnings.push(message);
          return new Promise(() => {});
        },
        showInformationMessage: (message) => {
          infos.push(message);
        },
      }),
      "restart command notification should not be awaited",
    );

    assert.equal(outcome, "disabled");
    assert.equal(disabledStatusUpdates, 1);
    assert.equal(restartCalls, 0);
    assert.match(warnings[0] ?? "", /language intelligence is disabled/);
    assert.deepEqual(infos, []);
  });

  it("does not show a success notification after restart failure", async () => {
    const warnings: string[] = [];
    const infos: string[] = [];

    const outcome = await runRestartLanguageServerCommand({
      settings: { enabled: true },
      updateDisabledStatus: () => {},
      runRestart: async () => {
        throw new Error("server start exploded");
      },
      showWarningMessage: (message) => {
        warnings.push(message);
      },
      showInformationMessage: (message) => {
        infos.push(message);
      },
    });

    assert.equal(outcome, "failed");
    assert.deepEqual(warnings, []);
    assert.deepEqual(infos, []);
  });

  it("shows success only after restart completes", async () => {
    const infos: string[] = [];

    const outcome = await settleWithin(
      runRestartLanguageServerCommand({
        settings: { enabled: true },
        updateDisabledStatus: () => {},
        runRestart: async () => {},
        showWarningMessage: () => {
          throw new Error("must not warn for enabled restart");
        },
        showInformationMessage: (message) => {
          infos.push(message);
          return new Promise(() => {});
        },
      }),
      "restart command notification should not be awaited",
    );

    assert.equal(outcome, "restarted");
    assert.deepEqual(infos, ["Merman language server restarted."]);
  });
});
