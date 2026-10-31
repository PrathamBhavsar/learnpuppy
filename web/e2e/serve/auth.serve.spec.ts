import { type ChildProcess, spawn } from "node:child_process"
import { mkdtempSync } from "node:fs"
import http from "node:http"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { expect, test } from "@playwright/test"

// Auth/security against a real, token-protected (non-dev) server: launch token -> cookie, 401 without it,
// Host 421, cross-Origin / missing-CSRF 403, no CORS headers. Spawns its own server on a free port.
const BIN =
  process.env["LP_BIN"] ??
  `${process.env["CARGO_TARGET_DIR"] ?? "../target"}/debug/learnpuppy`

let proc: ChildProcess
let port = 0
let token = ""

test.beforeAll(async () => {
  const dir = mkdtempSync(join(tmpdir(), "lp-auth-"))
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

const base = () => `http://127.0.0.1:${port}`

function rawGet(path: string, host: string): Promise<number> {
  return new Promise((resolve, reject) => {
    const req = http.request(
      { host: "127.0.0.1", port, path, headers: { Host: host } },
      (res) => {
        res.resume()
        resolve(res.statusCode ?? 0)
      }
    )
    req.on("error", reject)
    req.end()
  })
}

test("health is public; other API routes need the session cookie", async ({
  request,
}) => {
  expect((await request.get(`${base()}/api/v1/health`)).status()).toBe(200)
  const res = await request.get(`${base()}/api/v1/workspaces`)
  expect(res.status()).toBe(401)
  expect((await res.json()).error.code).toBe("auth_required")
})

test("a wrong launch token does not grant a session", async ({ request }) => {
  await request.get(`${base()}/?t=nope`, { maxRedirects: 0 })
  expect((await request.get(`${base()}/api/v1/workspaces`)).status()).toBe(401)
})

test("the launch token trades for an HttpOnly SameSite=Strict cookie and the URL is cleaned", async ({
  page,
}) => {
  await page.goto(`${base()}/?t=${token}`)
  await expect(page).toHaveURL(`${base()}/`)
  const cookies = await page.context().cookies(base())
  const c = cookies.find((x) => x.name === "lp_session")
  expect(c?.httpOnly).toBe(true)
  expect(c?.sameSite).toBe("Strict")
  const res = await page.request.get(`${base()}/api/v1/workspaces`)
  expect(res.status()).toBe(200)
  expect(res.headers()["access-control-allow-origin"]).toBeUndefined()
})

test("wrong Host is refused with 421", async () => {
  expect(await rawGet("/api/v1/health", "evil.com:" + port)).toBe(421)
  expect(await rawGet("/api/v1/health", "localhost.evil.com:" + port)).toBe(421)
  expect(await rawGet("/api/v1/health", `127.0.0.1:${port}`)).toBe(200)
})

test("mutations need our own Origin and X-LP-CSRF", async ({ page }) => {
  await page.goto(`${base()}/?t=${token}`)
  const url = `${base()}/api/v1/workspaces`
  const body = { name: "x" }
  const cross = await page.request.post(url, {
    data: body,
    headers: { Origin: "http://evil.com", "X-LP-CSRF": "1" },
  })
  expect(cross.status()).toBe(403)
  const noCsrf = await page.request.post(url, {
    data: body,
    headers: { Origin: base() },
  })
  expect(noCsrf.status()).toBe(403)
  const noOrigin = await page.request.post(url, {
    data: body,
    headers: { "X-LP-CSRF": "1" },
  })
  expect(noOrigin.status()).toBe(403)
})

test("responses never leak credentials", async ({ page }) => {
  await page.goto(`${base()}/?t=${token}`)
  const accounts = await page.request.get(`${base()}/api/v1/accounts`)
  const text = await accounts.text()
  expect(text).not.toMatch(/accessToken|refreshToken|sk-ant|oauth_token/i)
})
