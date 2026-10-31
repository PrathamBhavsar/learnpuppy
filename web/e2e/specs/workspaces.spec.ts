import { mkdirSync } from "node:fs"
import { expect, test } from "@playwright/test"

test("create, rename, archive, delete and restore a workspace", async ({
  page,
}) => {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  await page.goto("/")
  await expect(page.getByTestId("home")).toBeVisible()

  // create
  await page.getByTestId("new-workspace").click()
  await page.getByTestId("ws-name-input").fill("E2E Zebra")
  await page.getByTestId("ws-submit").click()
  const created = page.locator('[data-testid^="workspace-item-"]', {
    hasText: "E2E Zebra",
  })
  await expect(created).toBeVisible()
  const id = (await created.getAttribute("data-testid"))!.replace(
    "workspace-item-",
    ""
  )
  const card = page.getByTestId(`workspace-item-${id}`)

  // rename
  await page.getByTestId(`workspace-menu-${id}`).click()
  await page.getByTestId(`workspace-rename-${id}`).click()
  await page.getByTestId("ws-name-input").fill("E2E Okapi")
  await page.getByTestId("ws-submit").click()
  await expect(card.getByText("E2E Okapi")).toBeVisible()

  // archive then hide it from the default list
  await page.getByTestId(`workspace-menu-${id}`).click()
  await page.getByTestId(`workspace-archive-${id}`).click()
  await expect(card).toHaveCount(0)

  // delete requires the typed name
  await page.getByTestId("archived-toggle").click()
  await expect(card).toBeVisible()
  await page.getByTestId(`workspace-menu-${id}`).click()
  await page.getByTestId(`workspace-delete-${id}`).click()
  await expect(page.getByTestId("delete-confirm")).toBeDisabled()
  await page.getByTestId("delete-confirm-input").fill("E2E Okapi")
  await page.getByTestId("delete-confirm").click()
  await expect(card).toHaveCount(0)

  // restore from trash
  await page.keyboard.press("Escape")
  await page.getByTestId("trash-link").click()
  const trashed = page
    .locator('[data-testid^="trash-item-"]')
    .filter({ hasText: "E2E Okapi" })
  await expect(trashed).toBeVisible()
  await trashed.locator('[data-testid^="trash-restore-"]').click()
  await expect(trashed).toHaveCount(0)

  expect(errors).toEqual([])
})

// Reference shots: LP_SHOTS=1 bunx playwright test workspaces
const SHOTS = [
  ["dark", "ltr"],
  ["light", "ltr"],
  ["dark", "rtl"],
  ["light", "rtl"],
] as const

for (const [theme, dir] of SHOTS) {
  test(`shots ${theme}-${dir}`, async ({ page }) => {
    test.skip(!process.env.LP_SHOTS, "set LP_SHOTS=1 to regenerate shots")
    mkdirSync("docs-screenshots", { recursive: true })
    await page.setViewportSize({ width: 1280, height: 800 })
    await page.emulateMedia({ reducedMotion: "reduce" })
    await page.addInitScript((t) => localStorage.setItem("lp-theme", t), theme)
    await page.goto(`/?dir=${dir}`)
    await expect(page.getByTestId("home")).toBeVisible()
    await page.waitForTimeout(500)
    await page.screenshot({
      path: `docs-screenshots/workspaces-home-${theme}-${dir}.png`,
    })
    await page.getByTestId("new-workspace").click()
    await expect(page.getByTestId("workspace-dialog")).toBeVisible()
    await page.waitForTimeout(400)
    await page.screenshot({
      path: `docs-screenshots/workspaces-dialog-${theme}-${dir}.png`,
    })
    await page.keyboard.press("Escape")
    await page.getByTestId("trash-link").click()
    await expect(page.getByTestId("screen-trash")).toBeVisible()
    await page.waitForTimeout(400)
    await page.screenshot({
      path: `docs-screenshots/workspaces-trash-${theme}-${dir}.png`,
    })
  })
}
