import { defineConfig, devices } from "@playwright/test"

// Runs e2e/serve/*.spec.ts against the real `learnpuppy serve --dev --engine faux` on a freshly seeded
// fixture (the SPA comes from web/dist, so run `bun run build` first).
//   LP_BIN   path to the learnpuppy binary (default: $CARGO_TARGET_DIR/debug/learnpuppy). Debug builds read
//            web/dist of the worktree they were compiled in, so with a target dir shared between worktrees
//            copy the binary after `cargo build -p lp-server` and point LP_BIN at the copy.
//   LP_PORT  port for the server (default 7931)
const PORT = Number(process.env["LP_PORT"] ?? 7931)
const BIN =
  process.env["LP_BIN"] ??
  `${process.env["CARGO_TARGET_DIR"] ?? "../target"}/debug/learnpuppy`
const FIXTURE = `/tmp/lp-e2e-fixture-${PORT}`

export default defineConfig({
  testDir: "./e2e/serve",
  // The server keeps state (chats, turns); one worker keeps specs independent of each other's data.
  workers: 1,
  fullyParallel: false,
  forbidOnly: Boolean(process.env["CI"]),
  reporter: [["list"]],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chrome",
      use: { ...devices["Desktop Chrome"], channel: "chrome" },
    },
  ],
  webServer: {
    command: `rm -rf ${FIXTURE} && ${BIN} seed --fixture small --out ${FIXTURE} && exec ${BIN} serve --dev --engine faux --fixture ${FIXTURE} --port ${PORT} --no-open`,
    url: `http://127.0.0.1:${PORT}/api/v1/health`,
    reuseExistingServer: false,
    timeout: 60_000,
  },
})
