import { expect, test } from "@playwright/test"

test("imports a note, persists its source message and serves the original artifact", async ({
  page,
}) => {
  await page.goto("/w/ws_5eed0001/learn/chat_done")
  await expect(page.getByRole("button", { name: "Import note" })).toBeEnabled()
  const upload = page.waitForResponse(
    (r) => r.url().includes("/sources/note?") && r.request().method() === "POST"
  )
  await page.getByTestId("composer-note-file").setInputFiles({
    name: "lesson.md",
    mimeType: "text/markdown",
    buffer: Buffer.from("# Lesson notes\nA value has one owner. 🐶"),
  })
  const response = await upload
  expect(response.status()).toBe(201)
  const body = (await response.json()) as { artifactId: string }
  await expect(
    page.getByText("Note imported. Ask the tutor about it.")
  ).toBeVisible()
  await expect(page.getByTestId("message-list")).toContainText(
    "A value has one owner."
  )
  await page.reload()
  await expect(page.getByTestId("message-list")).toContainText(
    "A value has one owner."
  )
  const artifact = await page.request.get(
    `/api/v1/workspaces/ws_5eed0001/artifacts/${body.artifactId}`
  )
  expect(artifact.ok()).toBe(true)
  expect(await artifact.text()).toBe(
    "# Lesson notes\nA value has one owner. 🐶"
  )
})
