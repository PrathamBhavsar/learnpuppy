import { mkdirSync } from "node:fs"
import { expect, test } from "@playwright/test"

// Regenerate the committed reference shots: LP_SHOTS=1 bunx playwright test screenshots
test.skip(
  !process.env.LP_SHOTS,
  "set LP_SHOTS=1 to regenerate docs-screenshots/"
)

const OUT = "docs-screenshots"

const SHOTS = [
  {
    name: "shell-dark-ltr",
    path: "/w/learnpuppy/graph",
    theme: "dark",
    dir: "ltr",
    height: 800,
  },
  {
    name: "shell-light-ltr",
    path: "/w/learnpuppy/graph",
    theme: "light",
    dir: "ltr",
    height: 800,
  },
  {
    name: "shell-dark-rtl",
    path: "/w/learnpuppy/graph",
    theme: "dark",
    dir: "rtl",
    height: 800,
  },
  {
    name: "shell-light-rtl",
    path: "/w/learnpuppy/graph",
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
