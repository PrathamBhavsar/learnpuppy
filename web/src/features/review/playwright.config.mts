import { defineConfig, devices } from "@playwright/test"

// Kept with F23's owned files. Run from web/: bunx playwright test -c src/features/review/playwright.config.mts
export default defineConfig({
  testDir: ".",
  testMatch: "keyboard.spec.mts",
  outputDir: "../../../test-results/review",
  workers: 1,
  reporter: "list",
  use: {
    ...devices["Desktop Chrome"],
    channel: "chrome",
    baseURL: "http://127.0.0.1:5183",
    trace: "retain-on-failure",
  },
  webServer: {
    command: "bun run dev:mock --host 127.0.0.1 --port 5183 --strictPort",
    cwd: "../../..",
    url: "http://127.0.0.1:5183",
    reuseExistingServer: true,
  },
})
