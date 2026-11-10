import { type Page, expect, test } from "@playwright/test"

// Visual baselines: screen x light/dark x LTR/RTL, threshold 0.1% (docs/web/07).
// The clock is pinned (a Sunday, UTC) so date-labelled charts are stable.
// Update with: bunx playwright test visual --update-snapshots
const WS = "ws_rustasync"
const SCREENS = [
  { name: "review", path: `/w/${WS}/review`, ready: "nav-review-badge" },
  {
    name: "learn",
    path: `/w/${WS}/learn/c_teach_quiz`,
    ready: "question-dock",
  },
  { name: "gallery", path: "/gallery", ready: "gallery-palette", height: 3900 },
  { name: "graph", path: `/w/${WS}/graph`, ready: "graph-counts" },
] as const

async function settle(page: Page, ready: string) {
  await expect(page.getByTestId(ready)).toBeVisible()
  await page.evaluate(() => document.fonts.ready)
  await page.waitForTimeout(400)
}

for (const screen of SCREENS) {
  for (const theme of ["light", "dark"] as const) {
    for (const dir of ["ltr", "rtl"] as const) {
      test(`visual ${screen.name} ${theme} ${dir}`, async ({ page }) => {
        await page.setViewportSize({
          width: 1280,
          height: "height" in screen ? screen.height : 800,
        })
        await page.emulateMedia({ reducedMotion: "reduce" })
        // The review forecast labels its 14 days from today; pin the date so baselines do not drift.
        await page.clock.setFixedTime(new Date("2026-11-01T12:00:00Z"))
        await page.addInitScript(
          (t) => localStorage.setItem("lp-theme", t),
          theme
        )
        await page.goto(`${screen.path}?dir=${dir}`)
        await settle(page, screen.ready)
        await expect(page).toHaveScreenshot(
          `${screen.name}-${theme}-${dir}.png`,
          {
            maxDiffPixelRatio: 0.001,
            animations: "disabled",
            // The force-directed canvas is covered by graph specs; mask it here.
            mask: [page.locator("canvas")],
          }
        )
      })
    }
  }
}
