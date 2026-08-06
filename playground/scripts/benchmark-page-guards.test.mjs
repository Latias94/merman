import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";

import {
  BenchmarkPageOperationError,
  cancelBenchmarkPage,
  runBenchmarkPageOperation,
  runBenchmarkStartupOperation,
} from "./benchmark-page-guards.mjs";

class FakePage extends EventEmitter {
  cancelReason = null;

  async evaluate(_operation, reason) {
    this.cancelReason = reason;
  }
}

class FakeBrowser extends EventEmitter {
  closed = false;

  async close() {
    if (this.closed) return;
    this.closed = true;
    this.emit("disconnected");
  }
}

test("whole-corpus deadline cancels discovery and closes its browser", async () => {
  const page = new FakePage();
  const browser = new FakeBrowser();

  await assert.rejects(
    runBenchmarkPageOperation({
      browser,
      deadlineMs: Date.now() + 10,
      operation: () => new Promise(() => {}),
      page,
    }),
    (error) =>
      error instanceof BenchmarkPageOperationError &&
      error.code === "cli-timeout"
  );
  assert.equal(page.cancelReason, "cli-timeout");
  assert.equal(browser.closed, true);
});

test("page crashes reject a pending discovery operation", async () => {
  const page = new FakePage();
  const browser = new FakeBrowser();
  const pending = runBenchmarkPageOperation({
    browser,
    deadlineMs: Date.now() + 1_000,
    operation: () => new Promise(() => {}),
    page,
  });

  page.emit("crash");
  await assert.rejects(
    pending,
    (error) =>
      error instanceof BenchmarkPageOperationError &&
      error.code === "browser-crash"
  );
});

test("page close and browser disconnect keep distinct failure codes", async () => {
  for (const [event, code] of [
    ["close", "browser-page-close"],
    ["disconnected", "browser-disconnected"],
  ]) {
    const page = new FakePage();
    const browser = new FakeBrowser();
    const pending = runBenchmarkPageOperation({
      browser,
      deadlineMs: Date.now() + 1_000,
      operation: () => new Promise(() => {}),
      page,
    });

    (event === "close" ? page : browser).emit(event);
    await assert.rejects(
      pending,
      (error) =>
        error instanceof BenchmarkPageOperationError && error.code === code
    );
  }
});

test("explicit cancellation reaches the page and closes the browser", async () => {
  const page = new FakePage();
  const browser = new FakeBrowser();

  await cancelBenchmarkPage(page, browser, "sigterm");

  assert.equal(page.cancelReason, "sigterm");
  assert.equal(browser.closed, true);
});

test("startup deadline rejects and runs its cleanup", async () => {
  let cleanedUp = false;

  await assert.rejects(
    runBenchmarkStartupOperation({
      deadlineMs: Date.now() + 10,
      onTimeout() {
        cleanedUp = true;
      },
      operation: () => new Promise(() => {}),
    }),
    (error) =>
      error instanceof BenchmarkPageOperationError &&
      error.code === "cli-timeout"
  );
  assert.equal(cleanedUp, true);
});
