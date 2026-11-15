import AxeBuilder from "@axe-core/playwright"
import { expect, test } from "@playwright/test"
import { fileURLToPath } from "node:url"

// The PDF the backend tests import (3 chapters, an outline). Under dev:mock the file only has to be a PDF:
// the in-browser mock "server" stands in for the import. VITE_API=real runs the same flow in e2e/serve.
const FIXTURE_PDF = fileURLToPath(
  new URL(
    "../../../crates/lp-service/tests/fixtures/textbook.pdf",
    import.meta.url
  )
)
const WS = "ws_rustasync"
const URL_ = `/w/${WS}/textbook`

test("upload a PDF, watch the import, then check a chapter by keyboard", async ({
  page,
}) => {
  await page.goto(URL_)
  await expect(page.getByText("No textbooks yet")).toBeVisible()

  await page.getByTestId("textbook-file").setInputFiles(FIXTURE_PDF)
  // Upload only maps the book: three chapters wait, no import has started.
  await expect(
    page.getByTestId("chapter-list").getByRole("listitem")
  ).toHaveCount(3)
  await expect(page.getByTestId("textbook-import")).toBeHidden()
  await expect(page.getByTestId("chapter-0-badge")).toHaveText("Not imported")
  // Opening a chapter imports that chapter.
  await page.getByTestId("chapter-0-import").click()
  await expect(page.getByTestId("chapter-0-check")).toBeVisible({
    timeout: 20_000,
  })
  await expect(page.getByTestId("textbook-import")).toBeHidden({
    timeout: 20_000,
  })
  await expect(page.getByTestId("chapter-1-badge")).toHaveText("Not imported")
  await expect(page.getByTestId("chapter-0")).toContainText("Pages 1 to 12")
  await expect(page.getByTestId("chapter-0-badge")).toHaveText("Not read")

  await page.getByTestId("chapter-0-check").focus()
  await page.keyboard.press("Enter")
  await page.getByTestId("reading-check-start").click()
  await expect(page.getByTestId("reading-check-q")).toBeVisible()
  await page.getByRole("radio", { name: "Nothing" }).check()
  await page.getByTestId("reading-check-answer").focus()
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("reading-check-feedback")).toHaveAttribute(
    "data-correct",
    "true"
  )
  await expect(page.getByTestId("reading-check-next")).toBeFocused()
  await page.keyboard.press("Enter")
  await page.getByLabel("Your answer").fill("poll")
  await page.keyboard.press("Tab")
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("reading-check-feedback")).toHaveAttribute(
    "data-correct",
    "true"
  )
  await page.keyboard.press("Enter")
  await expect(page.getByTestId("reading-check-done")).toContainText(
    "2 of 2 right"
  )
  await page.getByRole("button", { name: "Back to chapters" }).click()
  await expect(page.getByTestId("chapter-0-badge")).toHaveText("Checked")
})

for (const theme of ["light", "dark"] as const) {
  test(`axe: textbook screens (${theme}) have no serious or critical violations`, async ({
    page,
  }) => {
    await page.addInitScript((t) => localStorage.setItem("lp-theme", t), theme)
    await page.goto(URL_)
    await page.getByTestId("textbook-file").setInputFiles(FIXTURE_PDF)
    await page.getByTestId("chapter-0-import").click()
    await expect(page.getByTestId("chapter-0-check")).toBeVisible({
      timeout: 20_000,
    })
    await expect(page.getByTestId("textbook-import")).toBeHidden({
      timeout: 20_000,
    })
    await expect(page.locator("html")).toHaveClass(new RegExp(theme))
    const scan = async () => {
      await page.waitForTimeout(400)
      const { violations } = await new AxeBuilder({ page })
        .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
        .analyze()
      expect(
        violations
          .filter((v) => v.impact === "serious" || v.impact === "critical")
          .map((v) => `${v.id}: ${v.nodes[0]?.target.join(" ")}`)
      ).toEqual([])
    }
    await scan() // the chapter list
    await page.getByTestId("chapter-0-check").click()
    await page.getByTestId("reading-check-start").click()
    await expect(page.getByTestId("reading-check-q")).toBeVisible()
    await scan() // a question
  })
}

test("axe: the list and the empty state", async ({ page }) => {
  await page.goto(URL_)
  await expect(page.getByText("No textbooks yet")).toBeVisible()
  const { violations } = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"])
    .analyze()
  expect(
    violations.filter((v) => v.impact === "serious" || v.impact === "critical")
  ).toEqual([])
})
