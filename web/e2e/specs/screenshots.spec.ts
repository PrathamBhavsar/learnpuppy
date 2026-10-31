import { mkdirSync } from "node:fs"
import { type Page, expect, test } from "@playwright/test"

// Regenerate the committed reference shots: LP_SHOTS=1 bunx playwright test screenshots
test.skip(
  !process.env.LP_SHOTS,
  "set LP_SHOTS=1 to regenerate docs-screenshots/"
)

const OUT = "docs-screenshots"

const SHOTS = [
  {
    name: "shell-dark-ltr",
    path: "/w/ws_rustasync/graph",
    theme: "dark",
    dir: "ltr",
    height: 800,
  },
  {
    name: "shell-light-ltr",
    path: "/w/ws_rustasync/graph",
    theme: "light",
    dir: "ltr",
    height: 800,
  },
  {
    name: "shell-dark-rtl",
    path: "/w/ws_rustasync/graph",
    theme: "dark",
    dir: "rtl",
    height: 800,
  },
  {
    name: "shell-light-rtl",
    path: "/w/ws_rustasync/graph",
    theme: "light",
    dir: "rtl",
    height: 800,
  },
  {
    name: "gallery-dark-ltr",
    path: "/gallery",
    theme: "dark",
    dir: "ltr",
    height: 3900,
  },
  {
    name: "gallery-light-ltr",
    path: "/gallery",
    theme: "light",
    dir: "ltr",
    height: 3900,
  },
  {
    name: "gallery-dark-rtl",
    path: "/gallery",
    theme: "dark",
    dir: "rtl",
    height: 3900,
  },
  {
    name: "gallery-light-rtl",
    path: "/gallery",
    theme: "light",
    dir: "rtl",
    height: 3900,
  },
] as const

test.beforeAll(() => mkdirSync(OUT, { recursive: true }))

for (const shot of SHOTS) {
  test(shot.name, async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: shot.height })
    await page.emulateMedia({ reducedMotion: "reduce" })
    await page.addInitScript(
      (theme) => localStorage.setItem("lp-theme", theme),
      shot.theme
    )
    await page.goto(`${shot.path}?dir=${shot.dir}`)
    await expect(
      shot.path === "/gallery"
        ? page.getByTestId("gallery-palette")
        : page.getByTestId("empty-state")
    ).toBeVisible()
    await page.waitForTimeout(400)
    await page.screenshot({ path: `${OUT}/${shot.name}.png` })
  })
}

type MockWindow = { __lpMock?: { dropSse: () => void } }

const OVERLAYS = [
  {
    name: "palette",
    open: (page: Page) => page.keyboard.press("ControlOrMeta+k"),
  },
  {
    name: "switcher",
    open: (page: Page) => page.getByTestId("workspace-chip").click(),
  },
  { name: "shortcuts", open: (page: Page) => page.keyboard.press("Shift+?") },
  {
    name: "banner",
    open: async (page: Page) => {
      await page.evaluate(() => (window as MockWindow).__lpMock?.dropSse())
      await page.getByTestId("connection-banner").waitFor({ timeout: 6000 })
    },
  },
] as const

for (const overlay of OVERLAYS) {
  for (const [theme, dir] of [
    ["dark", "ltr"],
    ["light", "ltr"],
    ["dark", "rtl"],
  ] as const) {
    test(`${overlay.name}-${theme}-${dir}`, async ({ page }) => {
      await page.setViewportSize({ width: 1280, height: 800 })
      await page.emulateMedia({ reducedMotion: "reduce" })
      await page.addInitScript(
        (t) => localStorage.setItem("lp-theme", t),
        theme
      )
      await page.goto(`/w/ws_rustasync/review?dir=${dir}`)
      await expect(page.getByTestId("nav-review-badge")).toBeVisible()
      await overlay.open(page)
      await page.waitForTimeout(500)
      await page.screenshot({
        path: `${OUT}/${overlay.name}-${theme}-${dir}.png`,
      })
    })
  }
}
