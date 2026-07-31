import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";

import {
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
    /whole-corpus CLI timeout/u
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
  await assert.rejects(pending, /Benchmark page crashed/u);
});

test("explicit cancellation reaches the page and closes the browser", async () => {
  const page = new FakePage();
  const browser = new FakeBrowser();

  await cancelBenchmarkPage(page, browser, "sigterm");

  assert.equal(page.cancelReason, "sigterm");
  assert.equal(browser.closed, true);
});

test("startup deadline rejects and disposes a resource that resolves late", async () => {
  const startup = Promise.withResolvers();
  const disposed = [];
  const running = runBenchmarkStartupOperation({
    deadlineMs: Date.now() + 10,
    disposeLateResult(resource) {
      disposed.push(resource);
    },
    operation: () => startup.promise,
  });

  await assert.rejects(running, /whole-corpus CLI timeout/u);
  startup.resolve("late-browser");
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(disposed, ["late-browser"]);
});

test("one startup abort rejects before a browser handle exists", async () => {
  const abort = new AbortController();
  const running = runBenchmarkStartupOperation({
    deadlineMs: Date.now() + 1_000,
    operation: () => new Promise(() => {}),
    signal: abort.signal,
  });

  abort.abort("sigint");
  await assert.rejects(running, /Browser corpus interrupted: sigint/u);
});
