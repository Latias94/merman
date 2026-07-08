import * as cp from "node:child_process";
import type { Writable } from "node:stream";

interface ClipboardChildProcess {
  stdin: Writable | null;
  exitCode: number | null;
  signalCode: NodeJS.Signals | null;
  kill(signal?: NodeJS.Signals): boolean;
  on(event: "error", listener: (error: Error) => void): this;
  on(event: "close", listener: (code: number | null, signal: NodeJS.Signals | null) => void): this;
}

type ClipboardCommandSpawner = (
  command: string,
  args: readonly string[],
  options: cp.SpawnOptions,
) => ClipboardChildProcess;

export interface ClipboardCommandOptions {
  timeoutMs?: number;
  killGraceMs?: number;
  spawnProcess?: ClipboardCommandSpawner;
}

export function runClipboardCommand(
  command: string,
  args: readonly string[],
  stdin?: Buffer,
  options: ClipboardCommandOptions = {},
): Promise<void> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let timedOut = false;
    let timeoutTimer: NodeJS.Timeout | undefined;
    let killTimer: NodeJS.Timeout | undefined;
    const clearTimers = (): void => {
      if (timeoutTimer) {
        clearTimeout(timeoutTimer);
        timeoutTimer = undefined;
      }
      if (killTimer) {
        clearTimeout(killTimer);
        killTimer = undefined;
      }
    };
    const rejectOnce = (error: Error): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimers();
      reject(error);
    };
    const resolveOnce = (): void => {
      if (settled) {
        return;
      }
      settled = true;
      clearTimers();
      resolve();
    };
    let child: ClipboardChildProcess;
    try {
      child = (options.spawnProcess ?? spawnClipboardProcess)(command, args, {
        stdio: stdin ? "pipe" : "ignore",
        windowsHide: true,
      });
    } catch (error) {
      rejectOnce(error instanceof Error ? error : new Error(String(error)));
      return;
    }
    timeoutTimer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
      killTimer = setTimeout(() => {
        if (!settled && child.exitCode === null && child.signalCode === null) {
          child.kill("SIGKILL");
        }
      }, options.killGraceMs ?? 1000);
    }, options.timeoutMs ?? 30000);
    child.on("error", rejectOnce);
    child.on("close", (code) => {
      if (timedOut) {
        rejectOnce(new Error(`${command} timed out.`));
        return;
      }
      if (code === 0) {
        resolveOnce();
      } else {
        rejectOnce(new Error(`${command} exited with status ${code ?? "unknown"}`));
      }
    });
    child.stdin?.on("error", rejectOnce);
    if (stdin) {
      child.stdin?.end(stdin);
    }
  });
}

function spawnClipboardProcess(
  command: string,
  args: readonly string[],
  options: cp.SpawnOptions,
): ClipboardChildProcess {
  return cp.spawn(command, [...args], options);
}
