import AxeBuilder from "@axe-core/playwright"
import { type Page, expect, test } from "@playwright/test"

const SHELL = "/w/ws_rustasync/graph"

async function collectErrors(page: Page) {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()))
  return errors
}

async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze()
  return violations
    .filter((v) => v.impact === "serious" || v.impact === "critical")
    .map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
}

for (const theme of ["dark", "light"] as const) {
  for (const dir of ["ltr", "rtl"] as const) {
    test(`shell and gallery are axe-clean (${theme}, ${dir})`, async ({
      page,
    }) => {
      await page.addInitScript(
        (t) => localStorage.setItem("lp-theme", t),
        theme
      )
      const errors = await collectErrors(page)
      await page.goto(`${SHELL}?dir=${dir}`)
      await expect(page.getByTestId("empty-state")).toBeVisible()
      expect(await seriousViolations(page)).toEqual([])

      await page.goto(`/gallery?dir=${dir}`)
      await expect(page.getByTestId("gallery-palette")).toBeAttached()
      expect(await seriousViolations(page)).toEqual([])
      expect(errors).toEqual([])
    })
  }
}

test("the stored theme is applied before first paint (no flash)", async ({
  page,
}) => {
  await page.addInitScript(() => localStorage.setItem("lp-theme", "light"))
  await page.goto(SHELL, { waitUntil: "commit" })
  await page.waitForFunction(
    () => document.documentElement.classList.length > 0
  )
  const early = await page.evaluate(() => ({
    cls: document.documentElement.className,
    scheme: document.documentElement.style.colorScheme,
  }))
  expect(early).toEqual({ cls: "light", scheme: "light" })
})

test("theme switch persists across reload", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" })
  await page.goto(SHELL)
  await expect(page.locator("html")).toHaveClass(/dark/)
  await page.getByTestId("theme-toggle").click()
  await page.getByTestId("theme-option-light").click()
  await expect(page.locator("html")).toHaveClass(/light/)
  await page.reload()
  await expect(page.locator("html")).toHaveClass(/light/)
})

test("?dir=rtl mirrors the shell: sidebar sits at the inline-start (right) edge", async ({
  page,
}) => {
  await page.goto(`${SHELL}?dir=rtl`)
  await expect(page.locator("html")).toHaveAttribute("dir", "rtl")
  const sidebar = await page.getByTestId("sidebar").boundingBox()
  const main = await page.getByTestId("main").boundingBox()
  expect(sidebar && main && sidebar.x > main.x).toBe(true)
  await page.getByTestId("direction-toggle").click()
  await expect(page.locator("html")).toHaveAttribute("dir", "ltr")
})

test("[ collapses the sidebar to 56 px and expands it back to 232 px", async ({
  page,
}) => {
  await page.goto(SHELL)
  const sidebar = page.getByTestId("sidebar")
  await expect.poll(async () => (await sidebar.boundingBox())?.width).toBe(232)
  await page.keyboard.press("[")
  await expect
    .poll(async () => Math.round((await sidebar.boundingBox())?.width ?? 0))
    .toBe(56)
  await page.keyboard.press("[")
  await expect
    .poll(async () => Math.round((await sidebar.boundingBox())?.width ?? 0))
    .toBe(232)
})

test("reduced motion leaves no running animation longer than 150 ms", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" })
  await page.goto(SHELL)
  await expect(page.getByTestId("empty-state")).toBeVisible()
  await page.waitForTimeout(300)
  const long = await page.evaluate(() =>
    document
      .getAnimations()
      .filter((a) => a.playState === "running")
      .map((a) => Number(a.effect?.getComputedTiming().activeDuration ?? 0))
      .filter((d) => d > 150)
  )
  expect(long).toEqual([])
})

test("command palette opens with the search trigger and navigates", async ({
  page,
}) => {
  const errors = await collectErrors(page)
  await page.goto(SHELL)
  await page.getByTestId("search-trigger").click()
  await expect(page.getByRole("dialog")).toBeVisible()
  expect(await seriousViolations(page)).toEqual([])
  await page.getByTestId("palette-cmd-go-review").click()
  await expect(page).toHaveURL(/\/w\/ws_rustasync\/review$/)
  expect(errors).toEqual([])
})
