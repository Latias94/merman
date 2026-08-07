import path from "node:path";

import { defineConfig, devices } from "@playwright/test";

const playgroundRoot = path.resolve(import.meta.dirname, "..");
const host = "127.0.0.1";
const port = Number(process.env.PLAYWRIGHT_PORT ?? 4178);
const baseURL = `http://${host}:${port}/merman/`;

export default defineConfig({
  testDir: ".",
  outputDir: path.join(playgroundRoot, "test-results"),
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: process.env.CI
    ? [
        ["line"],
        [
          "html",
          {
            open: "never",
            outputFolder: path.join(playgroundRoot, "playwright-report"),
          },
        ],
      ]
    : "list",
  use: {
    baseURL,
    locale: "en-US",
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  expect: {
    timeout: 10_000,
  },
  webServer: {
    command: `npm run preview -- --host ${host} --port ${port} --strictPort`,
    cwd: playgroundRoot,
    url: baseURL,
    reuseExistingServer: false,
    timeout: 30_000,
  },
  projects: [
    {
      name: "chromium-desktop",
      testIgnore: [
        /cross-browser\.smoke\.spec\.ts/u,
        /mobile\.interactions\.spec\.ts/u,
      ],
      use: { ...devices["Desktop Chrome"] },
    },
    {
      name: "chromium-mobile-interactions",
      testMatch: /mobile\.interactions\.spec\.ts/u,
      use: {
        ...devices["Pixel 7"],
        permissions: ["clipboard-read", "clipboard-write"],
      },
    },
    {
      name: "firefox-smoke",
      testMatch: /cross-browser\.smoke\.spec\.ts/u,
      use: { ...devices["Desktop Firefox"] },
    },
    {
      name: "webkit-smoke",
      testMatch: /cross-browser\.smoke\.spec\.ts/u,
      use: { ...devices["Desktop Safari"] },
    },
  ],
});
