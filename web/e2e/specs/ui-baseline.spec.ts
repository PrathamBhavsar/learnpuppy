import { mkdirSync } from "node:fs"
import { type Page, expect, test } from "@playwright/test"

// U0 baseline screenshots for manual before/after comparison (brain/ui-polish).
// Output goes to e2e/.shots/ (gitignored); nothing is asserted against pixels.
const WS = "ws_rustasync"
const OUT = "e2e/.shots"

const SCREENS: {
  name: string
  path: string
  open?: (page: Page) => Promise<void>
  ready: string
}[] = [
  {
    name: "classroom",
    path: `/w/${WS}/learn/c_teach_quiz`,
    ready: "question-dock",
  },
  {
    name: "course-lesson",
    path: `/w/${WS}/course`,
    ready: "course-view",
    // The mock seeds no courses; start one through the in-browser API, then open it.
    open: async (page) => {
      // The course list only renders once MSW is serving, so wait for it first.
      await expect(page.getByTestId("course-list")).toBeVisible()
      const id = await page.evaluate(async (ws) => {
        const res = await fetch(`/api/v1/workspaces/${ws}/courses`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ goal: "Learn Rust async" }),
        })
        return ((await res.json()) as { id: string }).id
      }, WS)
      await page.goto(`/w/${WS}/course?id=${id}`)
    },
  },
  { name: "digest", path: `/w/${WS}/digest`, ready: "digest-controls" },
]

const VIEWPORTS = [
  { label: "1440", width: 1440, height: 900 },
  { label: "390", width: 390, height: 844 },
] as const

test.beforeAll(() => mkdirSync(OUT, { recursive: true }))

for (const screen of SCREENS) {
  for (const vp of VIEWPORTS) {
    for (const theme of ["light", "dark"] as const) {
      test(`baseline ${screen.name} ${vp.label} ${theme}`, async ({ page }) => {
        await page.setViewportSize({ width: vp.width, height: vp.height })
        await page.emulateMedia({ reducedMotion: "reduce" })
        await page.addInitScript(
          (t) => localStorage.setItem("lp-theme", t),
          theme
        )
        await page.goto(screen.path)
        await screen.open?.(page)
        await expect(page.getByTestId(screen.ready)).toBeVisible()
        await page.evaluate(() => document.fonts.ready)
        await page.waitForTimeout(400)
        await page.screenshot({
          path: `${OUT}/${screen.name}-${vp.label}-${theme}.png`,
          fullPage: true,
        })
      })
    }
  }
}
