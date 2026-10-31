import AxeBuilder from "@axe-core/playwright"
import { type Page, expect, test } from "@playwright/test"

import {
  GRAPH,
  type LpWindow,
  cam,
  clearNodes,
  count,
  listedIds,
  openGraph,
  selection,
  settle,
} from "../graph-helpers"

test("renders the mock graph with counts, legend and an accessible list", async ({ page }) => {
  const errors = await openGraph(page)
  expect(await count(page)).toBe(150)
  await expect(page.getByTestId("graph-counts")).toHaveText("150 nodes · 201 chains")
  await expect(page.getByTestId("graph-legend")).toBeVisible()
  await expect(page.getByTestId("graph-list")).toHaveAttribute("aria-label", /most connected first/i)
  await expect(page.getByRole("heading", { level: 1, name: "Graph" })).toBeAttached()
  expect(errors).toEqual([])
})

test("hover tags a node, click selects it and opens the drawer, Esc clears", async ({ page }) => {
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.move(a!.x, a!.y)
  await expect.poll(() => page.evaluate(() => (window as LpWindow).__lp?.graph?.hover())).toBe(a!.id)
  await expect(page.getByTestId("graph-node-tag")).toBeVisible()

  await page.mouse.click(a!.x, a!.y)
  await expect.poll(() => selection(page)).toEqual([a!.id])
  await expect(page.getByTestId("graph-drawer")).toBeVisible()
  await expect(page.getByTestId("graph-selection-count")).toHaveText("1 selected")
  await expect(page).toHaveURL(new RegExp(`node=${a!.id}`))
  await expect(page.getByTestId("graph-live")).toContainText("Selected:")

  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("Escape")
  await expect.poll(() => selection(page)).toEqual([])
  await expect(page.getByTestId("graph-drawer")).toBeHidden()
  await expect(page).not.toHaveURL(/node=/)
})

test("shift-click multi-selects and the selection bar counts them", async ({ page }) => {
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  await expect.poll(() => selection(page)).toEqual([a!.id])
  await settle(page) // the camera flies to the node and makes room for the drawer
  const b = (await clearNodes(page, 3)).find((n) => n.id !== a!.id)!
  await page.keyboard.down("Shift")
  await page.mouse.click(b.x, b.y)
  await page.keyboard.up("Shift")
  await expect.poll(async () => (await selection(page)).length).toBe(2)
  await expect(page.getByTestId("graph-selection-count")).toHaveText("2 selected")
  expect(await selection(page)).toEqual(expect.arrayContaining([a!.id, b.id]))

  // The bar docks into the free area instead of sliding under the open drawer.
  await page.waitForTimeout(300)
  const bar = (await page.getByTestId("graph-selection-bar").boundingBox())!
  const drawer = (await page.getByTestId("graph-drawer").boundingBox())!
  expect(bar.x + bar.width).toBeLessThanOrEqual(drawer.x + 1)
  // ...and the legend lifts clear of the bar instead of sitting under it.
  const legend = (await page.getByTestId("graph-legend").boundingBox())!
  expect(legend.y + legend.height).toBeLessThanOrEqual(bar.y)

  await page.getByTestId("graph-selection-clear").click()
  await expect.poll(() => selection(page)).toEqual([])
})

test("shift-drag draws a marquee and selects what it covers", async ({ page }) => {
  await openGraph(page)
  const box = (await page.getByTestId("graph-canvas").boundingBox())!
  const x0 = box.x + box.width * 0.3
  const y0 = box.y + box.height * 0.3
  await page.keyboard.down("Shift")
  await page.mouse.move(x0, y0)
  await page.mouse.down()
  await page.mouse.move(x0 + box.width * 0.2, y0 + box.height * 0.2, { steps: 8 })
  await expect(page.getByTestId("graph-marquee")).toBeVisible()
  await page.mouse.up()
  await page.keyboard.up("Shift")
  await expect.poll(async () => (await selection(page)).length).toBeGreaterThan(1)
  await expect(page.getByTestId("graph-selection-bar")).toBeVisible()
})

test("wheel zooms, + and - step it, f fits, double-click zooms in", async ({ page }) => {
  await openGraph(page)
  const fit = (await cam(page))!
  const box = (await page.getByTestId("graph-canvas").boundingBox())!
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await page.mouse.wheel(0, -400)
  await expect.poll(async () => (await cam(page))!.k).toBeGreaterThan(fit.k * 1.2)

  const zoomed = (await cam(page))!.k
  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("-")
  await expect.poll(async () => (await cam(page))!.k).toBeLessThan(zoomed)
  await page.keyboard.press("+")
  await page.keyboard.press("+")
  await expect.poll(async () => (await cam(page))!.k).toBeGreaterThan(zoomed)

  await page.keyboard.press("f")
  await expect.poll(async () => (await cam(page))!.k).toBeCloseTo(fit.k, 1)

  await page.mouse.dblclick(box.x + 4, box.y + box.height - 4)
  await expect.poll(async () => (await cam(page))!.k).toBeGreaterThan(fit.k * 1.3)
})

test("dragging pans the camera", async ({ page }) => {
  await openGraph(page)
  const before = (await cam(page))!
  const box = (await page.getByTestId("graph-canvas").boundingBox())!
  const x = box.x + box.width * 0.6
  const y = box.y + box.height * 0.5
  await page.mouse.move(x, y)
  await page.mouse.down()
  await page.mouse.move(x - 160, y + 60, { steps: 10 })
  await page.mouse.up()
  await expect.poll(async () => (await cam(page))!.x).toBeLessThan(before.x - 100)
  expect((await cam(page))!.k).toBeCloseTo(before.k, 5)
})

test("keyboard: arrows walk to a neighbour, Tab cycles, Enter opens the drawer", async ({ page }) => {
  await openGraph(page)
  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("Tab") // first proposition by connection count
  const first = await page.evaluate(() => (window as LpWindow).__lp?.graph?.active())
  expect(first).toBeTruthy()
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("graph-drawer")).toBeVisible()
  await page.keyboard.press("Escape")
  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("Tab")
  await page.keyboard.press("ArrowRight")
  await page.keyboard.press("ArrowDown")
  const moved = await page.evaluate(() => (window as LpWindow).__lp?.graph?.active())
  expect(moved).toBeTruthy()
})

test("search dims the rest, lists matches, and writes ?q=", async ({ page }) => {
  await openGraph(page)
  await page.getByTestId("graph-search").fill("executor")
  await expect(page).toHaveURL(/q=executor/)
  await expect(page.getByTestId("graph-search-count")).toContainText("match")
  const total = await page.getByTestId("graph-list-item").first().getAttribute("aria-setsize")
  expect(Number(total)).toBeGreaterThan(0)
  expect(Number(total)).toBeLessThan(150)
  await page.getByTestId("graph-search-clear").click()
  await expect(page).not.toHaveURL(/q=/)
})

test("source filter narrows the graph and clears", async ({ page }) => {
  await openGraph(page)
  await page.getByTestId("graph-filter-source-git").click()
  await expect(page).toHaveURL(/src=/)
  await expect(page.getByTestId("graph-counts")).toContainText("of 150 nodes")
  await page.getByTestId("graph-clear-filters").click()
  await expect(page.getByTestId("graph-counts")).toHaveText("150 nodes · 201 chains")
})

test("Delete key previews the blast radius, needs the acknowledgement, then removes the node", async ({
  page,
}) => {
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  await expect.poll(() => selection(page)).toEqual([a!.id])
  await page.getByTestId("graph-focus").focus()
  await page.keyboard.press("Delete")

  const dialog = page.getByTestId("graph-delete-dialog")
  await expect(dialog).toBeVisible()
  await expect(page.getByTestId("graph-delete-summary")).toContainText(/chains? lose/)
  const confirm = page.getByTestId("graph-delete-confirm")
  await expect(confirm).toBeDisabled()
  await page.getByTestId("graph-delete-ack").click()
  await expect(confirm).toBeEnabled()
  await confirm.click()
  await expect(dialog).toBeHidden()
  await expect.poll(() => count(page)).toBe(149)
  await expect(page.getByTestId("graph-counts")).toContainText("149 nodes")
  expect(await selection(page)).toEqual([])
})

test("chain table opens, sorts, scopes to the selection and exports CSV", async ({ page }) => {
  await openGraph(page)
  await page.getByTestId("graph-toggle-table").click()
  const table = page.getByTestId("graph-table")
  await expect(table).toBeVisible()
  await expect(page.getByTestId("graph-table-scope")).toHaveText("All chains · 201")
  const group = page.getByTestId("graph-chain-row-l1").first()
  await expect(group).toBeVisible()

  await page.getByTestId("graph-table-sort-count").click()
  await expect(page.getByTestId("graph-table-sort-count")).toHaveAttribute("aria-pressed", "true")

  const wasOpen = await group.getAttribute("aria-expanded")
  await group.click()
  await expect(group).toHaveAttribute("aria-expanded", wasOpen === "true" ? "false" : "true")

  const download = page.waitForEvent("download")
  await page.getByTestId("graph-table-export").click()
  const file = await download
  expect(file.suggestedFilename()).toBe("graph-chains.csv")
  const stream = await file.createReadStream()
  const chunks: Buffer[] = []
  for await (const chunk of stream) chunks.push(chunk as Buffer)
  const csv = Buffer.concat(chunks).toString("utf8")
  expect(csv.split("\n")[0]).toBe("group,chain,kind,from_id,from_text,to_id,to_text")
  expect(csv.split("\n").length).toBeGreaterThan(50)

  // Selecting a node scopes the table to the chains it touches.
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  await expect(page.getByTestId("graph-table-scope")).toContainText("Chains of 1 selected proposition")
  await page.getByTestId("graph-table-close").click()
  await expect(table).toBeHidden()
})

test("drawer shows provenance and hops along a chain", async ({ page }) => {
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  const drawer = page.getByTestId("graph-drawer")
  await expect(drawer).toBeVisible()
  await expect(page.getByTestId("graph-drawer-text")).not.toBeEmpty()
  await expect(page.getByTestId("graph-drawer-source")).toBeVisible()
  // The toolbar wraps inside the free area instead of running underneath the drawer.
  await page.waitForTimeout(350)
  const toolbar = (await page.getByTestId("graph-toolbar").boundingBox())!
  const docked = (await drawer.boundingBox())!
  expect(toolbar.x + toolbar.width).toBeLessThanOrEqual(docked.x + 1)
  const hop = drawer.locator("[data-testid^='graph-drawer-chain-']").first()
  await hop.click()
  await expect.poll(async () => (await selection(page))[0]).not.toBe(a!.id)
  await expect(page).not.toHaveURL(new RegExp(`node=${a!.id}`))
  await page.getByTestId("graph-drawer-close").click()
  await expect(drawer).toBeHidden()
})

test("a cold load with ?node= opens that node's drawer", async ({ page }) => {
  await openGraph(page)
  const [id] = await listedIds(page, 1)
  await page.goto(`${GRAPH}?node=${id}`)
  await expect(page.getByTestId("graph-drawer")).toBeVisible()
  await expect.poll(() => selection(page)).toEqual([id])
})

test("toolbar toggles hulls, labels and minimap and the choice survives a reload", async ({ page }) => {
  await openGraph(page)
  await expect(page.getByTestId("graph-minimap")).toBeVisible()
  await page.getByTestId("graph-toggle-minimap").click()
  await expect(page.getByTestId("graph-minimap")).toBeHidden()
  await page.getByTestId("graph-toggle-hulls").click()
  await page.reload()
  await page.waitForFunction(() => (window as LpWindow).__lp?.graph?.nodeCount() ?? 0 > 0)
  await expect(page.getByTestId("graph-minimap")).toBeHidden()
  await expect(page.getByTestId("graph-toggle-hulls")).toHaveAttribute("aria-pressed", "false")
})

test("right-click opens the node context menu", async ({ page }) => {
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y, { button: "right" })
  const menu = page.getByTestId("graph-context-menu")
  await expect(menu).toBeVisible()
  await expect(page.getByTestId("graph-menu-practice")).toBeVisible()
  await expect(page.getByTestId("graph-menu-delete")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(menu).toBeHidden()
})

test("the accessible list mirrors selection and keyboard opens the drawer", async ({ page }) => {
  await openGraph(page)
  const list = page.getByTestId("graph-list")
  await list.focus()
  await expect(page.locator("[data-lp-graph-list]")).toBeVisible()
  await page.keyboard.press("ArrowDown")
  await expect.poll(async () => (await selection(page)).length).toBe(1)
  await expect(page.getByTestId("graph-list-item").nth(1)).toHaveAttribute("aria-selected", "true")
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("graph-drawer")).toBeVisible()
})

test("another workspace loads its own, smaller graph", async ({ page }) => {
  await openGraph(page, "/w/ws_distsys/graph")
  expect(await count(page)).toBe(40)
})

test("mock fixture generator: 12k nodes switches on performance mode", async ({ page }) => {
  await openGraph(page)
  await page.evaluate(() => (window as LpWindow).__lp?.graph?.loadFixture(12_000))
  await expect.poll(() => count(page), { timeout: 15_000 }).toBe(12_000)
  await expect(page.getByTestId("graph-performance-chip")).toContainText("Performance mode")
  await expect(page.getByTestId("graph-performance-toggle")).toHaveText("Use full detail")
  await page.getByTestId("graph-performance-toggle").click()
  await expect(page.getByTestId("graph-performance-chip")).toContainText("Large graph")
  await expect(page.getByTestId("graph-performance-toggle")).toHaveText("Performance mode")
})

for (const theme of ["dark", "light"] as const) {
  for (const dir of ["ltr", "rtl"] as const) {
    test(`graph screen with the drawer open is axe-clean (${theme}, ${dir})`, async ({ page }) => {
      await page.addInitScript(
        ([t, d]) => {
          localStorage.setItem("lp-theme", t as string)
          localStorage.setItem("lp-dir", d as string)
        },
        [theme, dir]
      )
      await openGraph(page)
      await expect(page.locator("html")).toHaveAttribute("dir", dir)
      await expect(page.getByTestId("graph-canvas")).toHaveAttribute("dir", "ltr")
      const [a] = await clearNodes(page, 1)
      await page.mouse.click(a!.x, a!.y)
      await expect(page.getByTestId("graph-drawer")).toBeVisible()
      await page.waitForTimeout(400)
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

test("RTL: the drawer docks to the inline-end (left) while the canvas stays ltr", async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem("lp-dir", "rtl"))
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  const drawer = page.getByTestId("graph-drawer")
  await expect(drawer).toBeVisible()
  await page.waitForTimeout(400)
  const stage = (await page.getByTestId("graph-stage").boundingBox())!
  const box = (await drawer.boundingBox())!
  expect(box.x + box.width / 2).toBeLessThan(stage.x + stage.width / 2)
})

test("reduced motion: the drawer still opens and the camera lands without animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" })
  await openGraph(page)
  const [a] = await clearNodes(page, 1)
  await page.mouse.click(a!.x, a!.y)
  const drawer = page.getByTestId("graph-drawer")
  await expect(drawer).toBeVisible()
  await expect.poll(() => selection(page)).toEqual([a!.id])
  // Reduced motion skips the animation, not the repositioning: the minimap still clears the drawer.
  await page.waitForTimeout(150)
  const minimap = (await page.getByTestId("graph-minimap").boundingBox())!
  expect(minimap.x + minimap.width).toBeLessThanOrEqual((await drawer.boundingBox())!.x + 1)
})

test.describe("performance budgets (docs/web/07)", () => {
  test.describe.configure({ mode: "serial" })

  async function panZoomStats(page: Page) {
    await page.evaluate(() => (window as LpWindow).__lp?.graph?.resetFrameStats())
    await page.evaluate(() => {
      const w = window as unknown as { __gaps: number[]; __raf: number }
      w.__gaps = []
      let last = performance.now()
      const tick = (t: number) => {
        w.__gaps.push(t - last)
        last = t
        w.__raf = requestAnimationFrame(tick)
      }
      w.__raf = requestAnimationFrame(tick)
    })
    const box = (await page.getByTestId("graph-canvas").boundingBox())!
    const cx = box.x + box.width / 2
    const cy = box.y + box.height / 2
    await page.mouse.move(cx, cy)
    await page.mouse.down()
    for (let i = 0; i < 100; i++)
      await page.mouse.move(cx + Math.sin(i / 10) * 220, cy + Math.cos(i / 10) * 120)
    await page.mouse.up()
    for (let i = 0; i < 30; i++) {
      await page.mouse.wheel(0, -120)
      await page.waitForTimeout(8)
    }
    for (let i = 0; i < 30; i++) {
      await page.mouse.wheel(0, 120)
      await page.waitForTimeout(8)
    }
    await page.waitForTimeout(400)
    return page.evaluate(() => {
      const w = window as unknown as { __gaps: number[]; __raf: number }
      cancelAnimationFrame(w.__raf)
      const gaps = w.__gaps.slice(2).sort((a, b) => a - b)
      const gapP95 = gaps[Math.min(gaps.length - 1, Math.floor(gaps.length * 0.95))] ?? 0
      return { gapP95, samples: gaps.length, draw: (window as LpWindow).__lp?.graph?.frameStats() }
    })
  }

  for (const size of [5000, 10_000]) {
    test(`${size} nodes: first paint within 400ms of data, pan/zoom p95 frame <= 20ms`, async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 })
      await openGraph(page)
      const first = await page.evaluate(async (n) => {
        const hook = () => (window as LpWindow).__lp!.graph!
        await hook().loadFixture(n)
        const dataAt = performance.now()
        while (hook().nodeCount() !== n) await new Promise((r) => setTimeout(r, 2))
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)))
        return performance.now() - dataAt
      }, size)
      expect(first).toBeLessThan(400)
      await settle(page)
      await page.waitForTimeout(2000) // load-in tweens finish before sampling
      const stats = await panZoomStats(page)
      test.info().annotations.push({
        type: `perf ${size}`,
        description: `first paint ${first.toFixed(0)}ms, rAF gap p95 ${stats.gapP95.toFixed(1)}ms, draw p95 ${stats.draw?.costP95.toFixed(1)}ms (${stats.samples} frames)`,
      })
      expect(stats.gapP95).toBeLessThanOrEqual(20)
      expect(stats.draw?.costP95 ?? 99).toBeLessThanOrEqual(20)
    })
  }
})
