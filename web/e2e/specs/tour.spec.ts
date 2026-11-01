import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"

// A brand-new browser: no `lp-tour` key, so the welcome splash and tour run (every other spec starts "seen").
test.use({ storageState: { cookies: [], origins: [] } })

test("first visit: splash, keyboard-only tour, persists across reload", async ({
  page,
}) => {
  await page.goto("/w/ws_rustasync/graph")
  await expect(page.getByTestId("tour-splash")).toBeVisible()
  await expect(page.getByTestId("tour-start")).toBeFocused()
  await page.keyboard.press("Enter")

  const card = page.getByTestId("tour-card")
  await expect(card).toBeVisible()
  await expect(page.getByTestId("tour-title")).toHaveText(
    "Welcome to LearnPuppy"
  )
  await expect(page.getByTestId("tour-next")).toBeFocused()

  // Arrow keys move forward and back.
  await page.keyboard.press("ArrowRight")
  await expect(page.getByTestId("tour-title")).toContainText("Workspaces")
  await expect(page.getByTestId("tour-spotlight")).toBeVisible()

  // Axe on a spotlight step.
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze()
  expect(
    violations.filter((v) => ["serious", "critical"].includes(v.impact ?? ""))
  ).toEqual([])

  await page.keyboard.press("ArrowLeft")
  await expect(page.getByTestId("tour-title")).toHaveText(
    "Welcome to LearnPuppy"
  )

  // Walk to the end with the keyboard only.
  for (let i = 0; i < 30; i += 1) {
    if ((await page.getByTestId("tour-next").textContent()) === "Finish") break
    await page.keyboard.press("ArrowRight")
  }
  await expect(page.getByTestId("tour-title")).toHaveText("You are ready")
  await page.keyboard.press("Enter")
  await expect(card).toBeHidden()

  await page.reload()
  await expect(page.getByTestId("workspace-chip")).toBeVisible()
  await page.waitForTimeout(1200)
  await expect(page.getByTestId("tour-splash")).toBeHidden()
  await expect(card).toBeHidden()
})

test("Escape skips and is remembered; ?notour=1 never starts it", async ({
  page,
}) => {
  await page.goto("/?notour=1")
  await expect(page.getByTestId("home")).toBeVisible()
  await page.waitForTimeout(1200)
  await expect(page.getByTestId("tour-splash")).toBeHidden()

  await page.goto("/")
  await expect(page.getByTestId("tour-splash")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("tour-splash")).toBeHidden()
  expect(await page.evaluate(() => localStorage.getItem("lp-tour"))).toBe(
    "done"
  )
})

test("replay from the Help menu, on the home screen", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("lp-tour", "done"))
  await page.goto("/")
  await page.getByTestId("help-menu").click()
  await page.getByTestId("help-replay-tour").click()
  await expect(page.getByTestId("tour-card")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("tour-card")).toBeHidden()
})
