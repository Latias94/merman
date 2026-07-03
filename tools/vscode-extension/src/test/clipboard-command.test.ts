import * as assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { Writable } from "node:stream";
import { describe, it } from "node:test";

import { runClipboardCommand } from "../clipboard-command.js";

describe("clipboard command runner", () => {
  it("rejects synchronous spawn failures", async () => {
    await assert.rejects(
      runClipboardCommand("clipboard-tool", [], undefined, {
        spawnProcess: () => {
          throw new Error("spawn failed");
        },
      }),
      /spawn failed/,
    );
  });

  it("rejects stdin pipe errors through the command promise", async () => {
    const child = new EventEmitter() as EventEmitter & {
      stdin: Writable;
      exitCode: number | null;
      signalCode: NodeJS.Signals | null;
      kill: () => boolean;
    };
    child.stdin = new Writable({
      write(_chunk, _encoding, callback) {
        callback(new Error("clipboard stdin closed"));
      },
    });
    child.exitCode = null;
    child.signalCode = null;
    child.kill = () => true;

    await assert.rejects(
      runClipboardCommand("clipboard-tool", [], Buffer.from("png"), {
        spawnProcess: () => child,
      }),
      /clipboard stdin closed/,
    );
  });
});
