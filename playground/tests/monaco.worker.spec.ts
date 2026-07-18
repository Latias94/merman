import { expect, test } from "@playwright/test";
import {
  monitorBrowserErrors,
  openPlayground,
  replaceEditorSource,
} from "./helpers/playground";

test("Monaco and the Rust editor session start only local production workers", async ({
  page,
}) => {
  const errors = monitorBrowserErrors(page);
  const requests: string[] = [];
  const workers: string[] = [];
  page.on("request", (request) => requests.push(request.url()));
  page.on("worker", (worker) => workers.push(worker.url()));

  await openPlayground(page);
  await expect(page.getByRole("textbox", { name: "Editor content" })).toBeVisible();
  await expect
    .poll(() => workers.some((url) => /merman-language\.worker/i.test(url)))
    .toBe(true);
  await expect
    .poll(
      () =>
        new Set(
          requests.filter((url) => /merman_wasm_bg-[\w-]+\.wasm(?:\?|$)/.test(url))
        ).size
    )
    .toBeGreaterThanOrEqual(2);
  await expect
    .poll(() => workers.some((url) => /editor\.worker/i.test(url)))
    .toBe(true);

  await replaceEditorSource(page, "flowchart TD\n  A -->");
  await expect
    .poll(() => page.locator(".monaco-editor .squiggly-error").count())
    .toBeGreaterThan(0);

  await page.getByRole("tab", { name: "Config", exact: true }).click();
  await expect
    .poll(() => workers.some((url) => /json\.worker/i.test(url)))
    .toBe(true);

  const pageOrigin = new URL(page.url()).origin;
  const external = requests.filter((url) => {
    const parsed = new URL(url);
    return (parsed.protocol === "http:" || parsed.protocol === "https:") &&
      parsed.origin !== pageOrigin;
  });
  expect(external).toEqual([]);
  expect(requests.some((url) => /cdn\.jsdelivr\.net/i.test(url))).toBe(false);
  for (const workerUrl of workers) {
    expect(new URL(workerUrl).origin).toBe(pageOrigin);
  }
  errors.assertNone();
});

test("Compare and Benchmark realm entries cannot reach Monaco", async ({ page }) => {
  for (const entry of ["compare-realm.html", "benchmark.html"]) {
    const requests: string[] = [];
    const workers: string[] = [];
    const recordRequest = (request: { url(): string }) => requests.push(request.url());
    const recordWorker = (worker: { url(): string }) => workers.push(worker.url());
    page.on("request", recordRequest);
    page.on("worker", recordWorker);

    await page.goto(`./${entry}`, { waitUntil: "networkidle" });

    expect(requests.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url))).toEqual([]);
    expect(workers.filter((url) => /monaco|editor\.worker|json\.worker/i.test(url))).toEqual([]);
    page.off("request", recordRequest);
    page.off("worker", recordWorker);
  }
});
