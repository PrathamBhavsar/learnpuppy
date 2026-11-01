import { type ChildProcess, spawn } from "node:child_process"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { expect, test } from "@playwright/test"

// Local-first guarantee: walking every top-level screen of a real server makes zero requests
// to any host other than the loopback server itself (no CDN, font, analytics or telemetry).
const BIN =
  process.env["LP_BIN"] ??
  `${process.env["CARGO_TARGET_DIR"] ?? "../target"}/debug/learnpuppy`
const WS = "ws_5eed0001"

let proc: ChildProcess
let port = 0
let token = ""

test.beforeAll(async () => {
  const dir = mkdtempSync(join(tmpdir(), "lp-local-"))
  const seed = spawn(BIN, ["seed", "--fixture", "small", "--out", dir])
  await new Promise((r) => seed.on("exit", r))
  proc = spawn(
    BIN,
    ["serve", "--engine", "faux", "--fixture", dir, "--port", "0", "--no-open"],
    { stdio: ["ignore", "pipe", "pipe"] }
  )
  const launch = await new Promise<{ port: number; token: string }>(
    (resolve, reject) => {
      let buf = ""
      proc.stdout?.on("data", (d: Buffer) => {
        buf += d.toString()
        const line = buf.split("\n").find((l) => l.startsWith("{"))
        if (line) resolve(JSON.parse(line))
      })
      proc.on("exit", () => reject(new Error("server exited")))
      setTimeout(() => reject(new Error("no launch line")), 30_000)
    }
  )
  port = launch.port
  token = launch.token
})

test.afterAll(() => {
  proc?.kill()
})

test("no screen makes a request that leaves 127.0.0.1", async ({ page }) => {
  const hosts = new Set<string>()
  page.on("request", (req) => {
    const url = new URL(req.url())
    if (url.protocol === "http:" || url.protocol === "https:") {
      hosts.add(url.host)
    }
  })
  await page.goto(`http://127.0.0.1:${port}/?t=${token}`)
  for (const screen of [
    "learn",
    "graph",
    "review",
    "digest",
    "ingest",
    "settings",
  ]) {
    await page.goto(`http://127.0.0.1:${port}/w/${WS}/${screen}`)
    await page.waitForLoadState("networkidle")
  }
  expect([...hosts]).toEqual([`127.0.0.1:${port}`])
})
