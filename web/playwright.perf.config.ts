import { defineConfig, devices } from "@playwright/test"

// `make perf`: budgets from docs/web/07 against the vite mock build (no backend). Serial: timing-sensitive.
const PORT = Number(process.env["LP_WEB_PORT"] ?? 5173)

export default defineConfig({
  testDir: "./e2e/perf",
  workers: 1,
  fullyParallel: false,
  reporter: [["list"]],
  use: { baseURL: `http://localhost:${PORT}` },
  projects: [
    {
      name: "chrome",
      use: {
        ...devices["Desktop Chrome"],
        channel: "chrome",
        launchOptions: {
          args: ["--enable-precise-memory-info", "--js-flags=--expose-gc"],
        },
      },
    },
  ],
  webServer: {
    command: `bun run dev:mock --port ${PORT}`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: true,
    timeout: 60_000,
  },
})
