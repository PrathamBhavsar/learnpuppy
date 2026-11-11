import { screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import type { components } from "@/api/schema"
import { server } from "@/test/msw/server"
import { previewList } from "@/test/msw/fixtures/review-contract"
import { state } from "@/test/msw/db"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

type Dto = components["schemas"]

const WS = "ws_rustasync"
const API = `*/api/v1/workspaces/${WS}/review`

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

describe("Recall review card", () => {
  it("types the answer, sends text without a choice key, and shows the server verdict", async () => {
    const base = state.db.cards.get(WS)![0]!
    const recall: Dto["QueueCardDto"] = {
      id: base.id,
      front: "Name the powerhouse of the cell",
      options: [],
      answerMode: "recall",
      kind: "card",
      due: base.due,
      state: base.state,
    }
    let sent: Dto["AnswerRequest"] | null = null
    server.use(
      http.get(`${API}/queue`, () =>
        HttpResponse.json({ items: [recall], dueNow: 1, now: state.db.now })
      ),
      http.post(`${API}/:card/answer`, async ({ request }) => {
        sent = (await request.json()) as Dto["AnswerRequest"]
        const preview = previewList(base, state.db.now)
        const body: Dto["AnswerDto"] = {
          correct: false,
          correctKey: "",
          answer: "mitochondria",
          explanation: "ATP is made there.",
          rating: 1,
          next: { due: preview[0]!.due, intervalDays: 0 },
          preview,
        }
        return HttpResponse.json(body)
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/review`)
    await user.click(await screen.findByTestId("review-start"))
    await screen.findByTestId("review-card")

    expect(screen.queryByTestId("review-choices")).not.toBeInTheDocument()
    await user.type(screen.getByTestId("review-recall-input"), "the nucleus")
    await user.click(screen.getByTestId("review-recall-submit"))

    expect(
      await screen.findByTestId("review-recall-verdict")
    ).toHaveTextContent("Not quite. Answer: mitochondria")
    expect(sent).toMatchObject({ text: "the nucleus" })
    expect(sent!.choiceKey ?? null).toBeNull()
    expect(await screen.findByTestId("review-rating-bar")).toBeInTheDocument()
  })

  it("still renders old cards without answerMode as multiple choice", async () => {
    const base = state.db.cards.get(WS)![0]!
    server.use(
      http.get(`${API}/queue`, () =>
        HttpResponse.json({
          items: [
            {
              id: base.id,
              front: base.front,
              options: base.options,
              kind: "card",
              due: base.due,
              state: base.state,
            },
          ],
          dueNow: 1,
          now: state.db.now,
        })
      )
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/review`)
    await user.click(await screen.findByTestId("review-start"))
    expect(await screen.findByTestId("review-choices")).toBeInTheDocument()
    expect(screen.queryByTestId("review-recall")).not.toBeInTheDocument()
  })
})
