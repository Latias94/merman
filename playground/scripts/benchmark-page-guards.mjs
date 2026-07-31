const WHOLE_CORPUS_TIMEOUT =
  "Browser corpus exceeded its whole-corpus CLI timeout.";

export async function cancelBenchmarkPage(page, browser, reason) {
  const operations = [];
  if (page) {
    operations.push(
      Promise.resolve().then(() =>
        page.evaluate((cancelReason) => {
          window.__MERMAN_BENCHMARK_CORPUS__?.cancel(cancelReason);
        }, reason)
      )
    );
  }
  if (browser) operations.push(Promise.resolve().then(() => browser.close()));
  await Promise.allSettled(operations);
}

export async function runBenchmarkStartupOperation({
  deadlineMs,
  disposeLateResult,
  operation,
  signal,
}) {
  const timeoutMs = deadlineMs - Date.now();
  if (timeoutMs <= 0) throw new Error(WHOLE_CORPUS_TIMEOUT);
  if (signal?.aborted) throw benchmarkInterruptError(signal.reason);

  let expired = false;
  let rejectTerminal;
  const terminalFailure = new Promise((_, reject) => {
    rejectTerminal = reject;
  });
  const onAbort = () => rejectTerminal(benchmarkInterruptError(signal.reason));
  signal?.addEventListener("abort", onAbort, { once: true });
  const timeout = setTimeout(
    () => rejectTerminal(new Error(WHOLE_CORPUS_TIMEOUT)),
    timeoutMs
  );
  const pending = Promise.resolve().then(operation);
  void pending.then(
    (result) => {
      if (!expired || !disposeLateResult) return;
      void Promise.resolve(disposeLateResult(result)).catch(() => {});
    },
    () => {}
  );

  try {
    return await Promise.race([pending, terminalFailure]);
  } catch (error) {
    expired = true;
    throw error;
  } finally {
    clearTimeout(timeout);
    signal?.removeEventListener("abort", onAbort);
  }
}

export async function runBenchmarkPageOperation({
  browser,
  deadlineMs,
  operation,
  page,
}) {
  const timeoutMs = deadlineMs - Date.now();
  if (timeoutMs <= 0) throw new Error(WHOLE_CORPUS_TIMEOUT);

  let settled = false;
  let rejectTerminal;
  const terminalFailure = new Promise((_, reject) => {
    rejectTerminal = reject;
  });
  const fail = (message) => {
    if (settled) return;
    settled = true;
    rejectTerminal(new Error(message));
  };
  const onCrash = () => fail("Benchmark page crashed.");
  const onClose = () =>
    fail("Benchmark page closed before producing evidence.");
  const onDisconnected = () =>
    fail("Benchmark browser disconnected before producing evidence.");
  page.once("crash", onCrash);
  page.once("close", onClose);
  browser.once("disconnected", onDisconnected);
  const timeout = setTimeout(() => {
    fail(WHOLE_CORPUS_TIMEOUT);
    void cancelBenchmarkPage(page, browser, "cli-timeout");
  }, timeoutMs);

  try {
    return await Promise.race([
      Promise.resolve().then(operation),
      terminalFailure,
    ]);
  } finally {
    settled = true;
    clearTimeout(timeout);
    page.off("crash", onCrash);
    page.off("close", onClose);
    browser.off("disconnected", onDisconnected);
  }
}

function benchmarkInterruptError(reason) {
  return new Error(`Browser corpus interrupted: ${String(reason ?? "signal")}`);
}
