import { type Page, expect, test } from "@playwright/test"

import { type LpWindow, clearNodes, openGraph, selection, settle } from "../graph-helpers"

// Regenerate the committed reference shots: LP_SHOTS=1 bunx playwright test graph-screenshots
test.skip(!process.env.LP_SHOTS, "set LP_SHOTS=1 to regenerate docs-screenshots/graph-*.png")

const OUT = "docs-screenshots"
type Theme = "dark" | "light"
type Dir = "ltr" | "rtl"

test.use({ viewport: { width: 1200, height: 720 } })

async function prepare(page: Page, theme: Theme, dir: Dir) {
  await page.addInitScript(
    ([t, d]) => {
      localStorage.setItem("lp-theme", t as string)
      localStorage.setItem("lp-dir", d as string)
    },
    [theme, dir]
  )
  await openGraph(page)
  await expect(page.locator("html")).toHaveAttribute("dir", dir)
}

const shot = (page: Page, name: string) => page.screenshot({ path: `${OUT}/graph-${name}.png` })

for (const theme of ["dark", "light"] as const) {
  for (const dir of ["ltr", "rtl"] as const) {
    test(`overview ${theme} ${dir}`, async ({ page }) => {
      await prepare(page, theme, dir)
      await page.waitForTimeout(600)
      await shot(page, `overview-${theme}-${dir}`)
    })
  }
}

test("hover neighbour dim and node tag", async ({ page }) => {
  await prepare(page, "dark", "ltr")
  const [a] = await clearNodes(page, 1)
  await page.mouse.move(a!.x, a!.y)
  await expect(page.getByTestId("graph-node-tag")).toBeVisible()
  await page.waitForTimeout(500)
  await shot(page, "hover-dark-ltr")
})

for (const [theme, dir] of [
  ["dark", "ltr"],
  ["light", "rtl"],
] as const) {
  test(`drawer ${theme} ${dir}`, async ({ page }) => {
    await prepare(page, theme, dir)
    const [a] = await clearNodes(page, 1)
    await page.mouse.click(a!.x, a!.y)
    await expect(page.getByTestId("graph-drawer")).toBeVisible()
    await expect(page.getByTestId("graph-drawer-source")).toBeVisible()
    await settle(page)
    await page.waitForTimeout(500)
    await shot(page, `drawer-${theme}-${dir}`)
  })
}

test("multi-select with the selection bar", async ({ page }) => {
  await prepare(page, "dark", "ltr")
  const box = (await page.getByTestId("graph-canvas").boundingBox())!
  const x0 = box.x + box.width * 0.28
  const y0 = box.y + box.height * 0.25
  await page.keyboard.down("Shift")
  await page.mouse.move(x0, y0)
  await page.mouse.down()
  await page.mouse.move(x0 + box.width * 0.22, y0 + box.height * 0.3, { steps: 8 })
  await page.mouse.up()
  await page.keyboard.up("Shift")
  await expect.poll(async () => (await selection(page)).length).toBeGreaterThan(1)
  await page.waitForTimeout(600)
  await shot(page, "multiselect-dark-ltr")
})

test("delete preview with blast radius", async ({ page }) => {
  await prepare(page, "dark", "ltr")
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  await expect.poll(() => selection(page)).toEqual([a!.id])
  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("Delete")
  await expect(page.getByTestId("graph-delete-summary")).toBeVisible()
  await page.waitForTimeout(500)
  await shot(page, "delete-dark-ltr")
})

test("chain table", async ({ page }) => {
  await prepare(page, "dark", "ltr")
  await page.getByTestId("graph-toggle-table").click()
  await expect(page.getByTestId("graph-chain-row-l1").first()).toBeVisible()
  await page.getByTestId("graph-chain-row-l1").first().click()
  await page.waitForTimeout(500)
  await shot(page, "table-dark-ltr")
})

test("5k nodes", async ({ page }) => {
  await prepare(page, "dark", "ltr")
  await page.evaluate(() => (window as LpWindow).__lp?.graph?.loadFixture(5000))
  await expect.poll(() => page.evaluate(() => (window as LpWindow).__lp?.graph?.nodeCount())).toBe(5000)
  await page.waitForTimeout(2200)
  await shot(page, "5k-dark-ltr")
})
