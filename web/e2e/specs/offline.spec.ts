import { expect, test } from "@playwright/test"

type MockWindow = {
  __lpMock?: { dropSse: () => void; restoreSse: () => void }
}
const SHELL = "/w/ws_rustasync/review"

test("SSE drop shows the banner; repeated drop/restore cycles each recover", async ({
  page,
}) => {
  await page.goto(SHELL)
  await expect(page.getByTestId("workspace-chip")).toHaveAccessibleName(
    "Workspace: Rust async"
  )
  const banner = page.getByTestId("connection-banner")
  for (let i = 0; i < 2; i++) {
    await page.evaluate(() => (window as MockWindow).__lpMock?.dropSse())
    await expect(banner).toContainText("Reconnecting", { timeout: 6000 })
    await page.evaluate(() => (window as MockWindow).__lpMock?.restoreSse())
    await expect(banner).toContainText("Back online", { timeout: 15_000 })
    await expect(banner).toBeHidden({ timeout: 5000 })
  }
})

test("browser offline keeps the shell usable and navigation works", async ({
  page,
  context,
}) => {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  await page.goto(SHELL)
  await expect(page.getByTestId("nav-review-badge")).toBeVisible()
  await context.setOffline(true)
  await page.keyboard.press("ControlOrMeta+k")
  await expect(page.getByRole("dialog")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(page.getByRole("dialog")).toBeHidden()
  await context.setOffline(false)
  await expect(page.getByTestId("nav-review-badge")).toBeVisible()
  expect(errors).toEqual([])
})
