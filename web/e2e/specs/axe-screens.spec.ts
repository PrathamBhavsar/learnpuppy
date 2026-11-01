import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"

// Acceptance H: zero serious/critical axe violations on every top-level screen, in light and dark.
const WS = "ws_rustasync"
const SCREENS = [
  ["home", "/", "home"],
  ["trash", "/trash", "screen-trash"],
  ["review", `/w/${WS}/review`, "screen-review"],
  ["review-all", "/review-all", "screen-review-all"],
  ["digest", `/w/${WS}/digest`, "digest-controls"],
] as const

for (const theme of ["light", "dark"] as const) {
  for (const [name, path, testId] of SCREENS) {
    test(`axe: ${name} (${theme}) has no serious or critical violations`, async ({
      page,
    }) => {
      await page.addInitScript(
        (t) => localStorage.setItem("lp-theme", t),
        theme
      )
      await page.goto(path)
      await expect(page.getByTestId(testId)).toBeVisible()
      await expect(page.locator("html")).toHaveClass(new RegExp(theme))
      await page.waitForTimeout(500)
      const { violations } = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze()
      expect(
        violations
          .filter((v) => v.impact === "serious" || v.impact === "critical")
          .map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
      ).toEqual([])
    })
  }
}
