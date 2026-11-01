import { type Page, expect, test } from "@playwright/test"

const SHELL = "/w/ws_rustasync/graph"

/** Mock-mode hooks installed by src/test/mock/browser.ts. */
type MockWindow = {
  __lp?: { sse: { state: () => string } }
  __lpMock?: { dropSse: () => void; restoreSse: () => void }
}

async function openShell(page: Page, path = SHELL) {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  await page.goto(path)
  await expect(page.getByTestId("workspace-chip")).toHaveAccessibleName(
    "Workspace: Rust async"
  )
  return errors
}

test("mock mode serves the shell without a backend", async ({ page }) => {
  const errors = await openShell(page)
  await expect(page.getByTestId("nav-review-badge")).toHaveText("7")
  await expect(page.getByTestId("accounts-pill")).toContainText("ready")
  expect(
    await page.evaluate(() => (window as MockWindow).__lp?.sse.state())
  ).toBe("open")
  expect(errors).toEqual([])
})

test("⌘K palette: type, arrow, enter navigates", async ({ page }) => {
  await openShell(page)
  await page.keyboard.press("ControlOrMeta+k")
  await expect(page.getByTestId("palette-input")).toBeFocused()
  await page.keyboard.type("go to dig")
  await page.keyboard.press("Enter")
  await expect(page).toHaveURL(/\/w\/ws_rustasync\/digest$/)
  await page.keyboard.press("ControlOrMeta+k")
  await expect(page.getByTestId("palette-group-recent")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("palette-input")).toBeHidden()
})

test("workspace switcher keeps the section and ⌘2 jumps", async ({ page }) => {
  await openShell(page, "/w/ws_rustasync/review")
  await page.getByTestId("workspace-chip").click()
  await expect(
    page.getByTestId("workspace-option-ws_designnotes")
  ).toContainText("12")
  await page.getByTestId("workspace-option-ws_distsys").click()
  await expect(page).toHaveURL(/\/w\/ws_distsys\/review$/)
  await page.keyboard.press("ControlOrMeta+1")
  await expect(page).toHaveURL(/\/w\/ws_rustasync\/review$/)
})

test("g chords and ? shortcuts sheet", async ({ page }) => {
  await openShell(page)
  await page.keyboard.press("g")
  await page.keyboard.press("l")
  await expect(page).toHaveURL(/\/learn$/)
  await page.keyboard.press("Shift+?")
  await expect(page.getByTestId("shortcuts-sheet")).toBeVisible()
  await expect(page.getByTestId("shortcut-go-review")).toBeVisible()
  await page.keyboard.press("Escape")
  await expect(page.getByTestId("shortcuts-sheet")).toBeHidden()
})

test("connection banner appears after 2 s of SSE downtime and clears on recovery", async ({
  page,
}) => {
  await openShell(page)
  await expect(page.getByTestId("connection-banner")).toBeHidden()
  await page.evaluate(() => (window as MockWindow).__lpMock?.dropSse())
  await expect(page.getByTestId("connection-banner")).toContainText(
    "Reconnecting… your work is safe",
    { timeout: 6000 }
  )
  await page.evaluate(() => (window as MockWindow).__lpMock?.restoreSse())
  await expect(page.getByTestId("connection-banner")).toContainText(
    "Back online",
    {
      timeout: 15_000,
    }
  )
  await expect(page.getByTestId("connection-banner")).toBeHidden({
    timeout: 5000,
  })
})

test("mod+1/mod+2 switch workspace and keep the section; two tabs stay independent", async ({
  page,
  context,
}) => {
  await page.goto("/w/ws_rustasync/review")
  await expect(page.getByTestId("workspace-chip")).toHaveAccessibleName(
    "Workspace: Rust async"
  )
  const other = await page.evaluate(async () => {
    const list = (await (await fetch("/api/v1/workspaces")).json()) as {
      items: { id: string }[]
    }
    return list.items.map((w) => w.id)
  })
  expect(other.length).toBeGreaterThan(1)
  await expect(async () => {
    await page.keyboard.press("ControlOrMeta+2")
    await expect(page).toHaveURL(new RegExp(`/w/${other[1]}/review$`), {
      timeout: 1000,
    })
  }).toPass()

  // A second tab opened on the first workspace is unaffected by the first tab's switch.
  const tab2 = await context.newPage()
  await tab2.goto(`/w/${other[0]}/graph`)
  await expect(tab2).toHaveURL(new RegExp(`/w/${other[0]}/graph$`))
  await expect(async () => {
    await page.keyboard.press("ControlOrMeta+1")
    await expect(page).toHaveURL(new RegExp(`/w/${other[0]}/review$`), {
      timeout: 1000,
    })
  }).toPass()
  await expect(tab2).toHaveURL(new RegExp(`/w/${other[0]}/graph$`))
  await expect(async () => {
    await tab2.keyboard.press("ControlOrMeta+2")
    await expect(tab2).toHaveURL(new RegExp(`/w/${other[1]}/graph$`), {
      timeout: 1000,
    })
  }).toPass()
  await expect(page).toHaveURL(new RegExp(`/w/${other[0]}/review$`))
})
