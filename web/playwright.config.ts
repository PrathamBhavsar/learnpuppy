import { defineConfig, devices } from "@playwright/test"

// `LP_WEB_PORT` lets parallel worktrees each run their own dev:mock server.
const PORT = Number(process.env["LP_WEB_PORT"] ?? 5173)

// System Chrome (no browser download). Runs against `bun run dev:mock` (in-browser MSW, no backend).
export default defineConfig({
  testDir: "./e2e/specs",
  fullyParallel: true,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chrome",
      use: { ...devices["Desktop Chrome"], channel: "chrome" },
    },
  ],
  webServer: {
    command: `bun run dev:mock --port ${PORT}`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: true,
    timeout: 60_000,
  },
})
