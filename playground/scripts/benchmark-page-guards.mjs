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
  onTimeout,
  operation,
}) {
  const timeoutMs = deadlineMs - Date.now();
  if (timeoutMs <= 0) throw new Error(WHOLE_CORPUS_TIMEOUT);

  const { promise: terminalFailure, reject } = Promise.withResolvers();
  const timeout = setTimeout(() => {
    void Promise.resolve(onTimeout?.()).catch(() => {});
    reject(new Error(WHOLE_CORPUS_TIMEOUT));
  }, timeoutMs);

  try {
    return await Promise.race([
      Promise.resolve().then(operation),
      terminalFailure,
    ]);
  } finally {
    clearTimeout(timeout);
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
  const { promise: terminalFailure, reject } = Promise.withResolvers();
  const fail = (message) => {
    if (settled) return;
    settled = true;
    reject(new Error(message));
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
