import { expect, test } from "@playwright/test"

// Cross-screen journeys (docs/web/10): each walks several screens via nav, palette and chords.
const WS = "ws_rustasync"

test("journey: graph -> review -> learn -> digest via sidebar and back", async ({
  page,
}) => {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  await page.goto(`/w/${WS}/graph`)
  await expect(page.getByTestId("graph-counts")).toBeVisible()
  await page.getByTestId("search-trigger").click()
  await page.getByTestId("palette-cmd-go-review").click()
  await expect(page).toHaveURL(new RegExp(`/w/${WS}/review$`))
  await page.goto(`/w/${WS}/learn`)
  await expect(page.getByTestId("start-learning")).toBeVisible()
  await page.getByTestId("chat-item-c_teach_quiz").getByRole("link").click()
  await expect(page.getByTestId("question-dock")).toBeVisible()
  await page.getByTestId("quiz-option-A").click()
  await page.getByTestId("quiz-submit").click()
  await expect(page.getByTestId("verdict-call_quiz_2")).toBeVisible()
  await page.goBack()
  await expect(page).toHaveURL(new RegExp(`/w/${WS}/learn$`))
  expect(errors).toEqual([])
})

test("journey: deep links cold-load every top-level screen without errors", async ({
  page,
}) => {
  const errors: string[] = []
  page.on("pageerror", (e) => errors.push(e.message))
  for (const path of [
    `/w/${WS}/graph`,
    `/w/${WS}/review`,
    `/w/${WS}/learn`,
    `/w/${WS}/digest`,
    `/w/${WS}/ingest`,
    `/w/${WS}/settings`,
    "/accounts",
    "/settings",
    "/trash",
    "/gallery",
  ]) {
    await page.goto(path)
    await expect(page.locator("main, [role=main]").first()).toBeVisible()
  }
  expect(errors).toEqual([])
})
