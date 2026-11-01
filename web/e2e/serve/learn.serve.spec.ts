import { expect, test } from "@playwright/test"

// Real server, scripted (faux) engine, seeded "small" fixture: Rust async is `ws_5eed0001` with one chat
// at each phase. The faux engine has no script, so any turn that reaches the model ends in
// `engine_unavailable`, which is exactly the error path the Classroom must handle.
const WS = "ws_5eed0001"
const LEARN = `/w/${WS}/learn`

test("lists the seeded chats with the server's phase and pending flags", async ({
  page,
}) => {
  await page.goto(LEARN)
  for (const id of [
    "chat_probe",
    "chat_teach",
    "chat_plan",
    "chat_practice",
    "chat_done",
  ])
    await expect(page.getByTestId(`chat-item-${id}`)).toBeVisible()
  await expect(
    page.getByTestId("chat-item-chat_probe").getByTestId("chat-pending-dot")
  ).toBeVisible()
  await expect(
    page.getByTestId("chat-item-chat_plan").getByTestId("chat-pending-dot")
  ).toHaveCount(0)
  await expect(
    page.getByTestId("chat-item-chat_practice").getByTestId("chat-phase")
  ).toHaveAccessibleName("Phase: Practice")
})

test("a teach chat renders the real ClassroomView: verdict, quiz dock, plan and jobs", async ({
  page,
}) => {
  await page.goto(`${LEARN}/chat_teach`)
  await expect(page.getByTestId("classroom-title")).toHaveText(
    "How wakers work"
  )
  await expect(page.locator("[data-testid^=verdict-]").first()).toHaveAttribute(
    "data-correct",
    "true"
  )
  await expect(page.getByTestId("question-dock")).toHaveAttribute(
    "data-tool",
    "quiz"
  )
  await expect(page.getByTestId("quiz-option-A")).toBeVisible()
  await expect(page.getByTestId("confidence")).toBeVisible()
  await expect(page.locator("[data-testid^=plan-node-]")).toHaveCount(3)
  await expect(
    page.locator("[data-testid^=plan-node-][data-status=current]")
  ).toHaveCount(1)

  await page.getByTestId("tab-jobs").click()
  await expect(page.getByTestId("job-job_pdf_failed")).toHaveAttribute(
    "data-status",
    "failed"
  )
  await expect(page.getByTestId("job-error-job_pdf_failed")).toContainText(
    "no extractable text"
  )
  await page.getByTestId("job-open-job_demo_done").click()
  const frame = page.getByTestId("artifact-html")
  await expect(frame).toHaveAttribute("sandbox", "")
  await expect(
    page.frameLocator("[data-testid=artifact-html]").locator("button")
  ).toHaveText("Wake")
})

test("answering sends the structured answer; the model failing shows Retry", async ({
  page,
}) => {
  await page.goto(`${LEARN}/chat_teach`)
  await page.getByTestId("question-dock").waitFor()
  await page.getByTestId("quiz-option-B").click()
  const request = page.waitForRequest(
    (r) => r.url().endsWith("/chats/chat_teach/turn") && r.method() === "POST"
  )
  const response = page.waitForResponse(
    (r) =>
      r.url().endsWith("/chats/chat_teach/turn") &&
      r.request().method() === "POST"
  )
  await page.getByTestId("quiz-submit").click()
  const body = (await request).postDataJSON() as Record<string, unknown>
  expect(body).toMatchObject({
    type: "answer",
    answer: { confidence: 3 },
  })
  expect((body["answer"] as { choice: string[] }).choice).toHaveLength(1)
  expect(typeof body["clientId"]).toBe("string")
  expect((await response).status()).toBe(202)
  // The answer is graded and shown; the scripted engine then has nothing to say.
  await expect(page.getByTestId("question-dock")).toBeHidden()
  await expect(page.getByTestId("turn-error")).toContainText(
    "scripted engine has no response left"
  )
  await expect(page.getByTestId("turn-retry")).toBeVisible()
  await page.getByTestId("turn-retry").click()
  await expect(page.getByTestId("turn-error")).toBeVisible()
})

test("a question answered elsewhere resyncs silently and says Question updated", async ({
  page,
  baseURL,
}) => {
  // Without the event stream the page cannot hear about the other tab's answer, so only the 409 can fix it.
  await page.route("**/api/v1/events*", (route) => route.abort())
  await page.goto(`${LEARN}/chat_probe`)
  const dock = page.getByTestId("question-dock")
  await expect(dock).toHaveAttribute("data-tool", "probe")
  const callId = (await dock.getAttribute("data-call-id")) ?? ""
  // Another tab answers the same question first.
  const other = await page.request.post(
    `/api/v1/workspaces/${WS}/chats/chat_probe/turn`,
    {
      headers: { "X-LP-CSRF": "1", Origin: baseURL ?? "" },
      data: {
        type: "answer",
        callId,
        answer: { choice: [], text: "from another tab" },
        clientId: "other-tab-1",
      },
    }
  )
  expect(other.status()).toBe(202)
  await page.getByTestId("probe-text").fill("from this tab")
  await page.keyboard.press("Enter")
  await expect(page.getByText("Question updated")).toBeVisible()
  await expect(dock).toBeHidden()
  await expect(page.getByTestId("message-list")).toContainText(
    "from another tab"
  )
})

test("a new chat from the empty state opens and reports the engine failure with Retry", async ({
  page,
}) => {
  await page.goto(LEARN)
  await page.getByTestId("start-topic").fill("Rust lifetimes")
  await page.getByTestId("start-submit").click()
  await expect(page).toHaveURL(/\/learn\/chat_[0-9a-f]+$/)
  await expect(page.getByTestId("classroom-title")).toHaveText("Rust lifetimes")
  await expect(page.getByTestId("turn-error")).toContainText(
    "scripted engine has no response left"
  )
  await expect(page.getByTestId("turn-retry")).toBeEnabled()
  await expect(page.getByTestId("chat-item-chat_probe")).toBeVisible()
})

test("a practice chat shows its problem; a done chat has no question and a composer", async ({
  page,
}) => {
  await page.goto(`${LEARN}/chat_practice`)
  await expect(page.getByTestId("question-dock")).toHaveAttribute(
    "data-tool",
    "problem"
  )
  await expect(page.getByTestId("problem-statement")).toBeVisible()
  await expect(page.getByTestId("phase-practice")).toHaveAttribute(
    "data-state",
    "active"
  )
  await page.goto(`${LEARN}/chat_done`)
  await expect(page.getByTestId("composer")).toBeVisible()
  await expect(page.getByTestId("question-dock")).toHaveCount(0)
  await expect(page.getByTestId("phase-teach")).toHaveAttribute(
    "data-state",
    "done"
  )
})

test("rename, export and delete use the real endpoints", async ({ page }) => {
  await page.goto(LEARN)
  await page.getByTestId("chat-menu-chat_plan").click()
  await page.getByRole("menuitem", { name: /Rename/ }).click()
  await page.getByTestId("rename-input").fill("Pinning notes")
  await page.getByTestId("rename-submit").click()
  await expect(page.getByTestId("chat-item-chat_plan")).toContainText(
    "Pinning notes"
  )

  const exported = await page.request.get(
    `/api/v1/workspaces/${WS}/chats/chat_plan/export.md`
  )
  expect(exported.headers()["content-type"]).toContain("text/markdown")
  expect(await exported.text()).toContain("Pinning notes")

  await page.getByTestId("chat-menu-chat_plan").click()
  await page.getByRole("menuitem", { name: /Delete/ }).click()
  await page.getByTestId("delete-confirm").click()
  await expect(page.getByTestId("chat-item-chat_plan")).toHaveCount(0)
})

test("an unknown chat is a not-found state, not a crash", async ({ page }) => {
  await page.goto(`${LEARN}/chat_missing`)
  await expect(page.getByText("Chat not found")).toBeVisible()
})
