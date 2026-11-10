import { act, screen, waitFor } from "@testing-library/react"
import { HttpResponse, http } from "msw"
import { beforeEach, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"
import { resolveRange } from "./range"

const WS = "ws_rustasync"
const PATH = `/w/${WS}/digest`
const API = `*/api/v1/workspaces/${WS}/digest`
beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

it("rejects impossible custom dates before making an API query", () => {
  expect(
    resolveRange("custom", { from: "2026-02-30", to: "2026-99-99" }, 0)
  ).toEqual({ from: undefined, to: undefined })
})

it("reads B20's day envelope and sends an exclusive end date", async () => {
  let end: string | null = null
  server.use(
    http.get(API, ({ request }) => {
      end = new URL(request.url).searchParams.get("to")
      return HttpResponse.json({
        days: state.db.digest.days,
        totalSessions: 4,
        totalDecisions: 6,
      })
    })
  )
  // The fixture's days are relative to today, so the custom range has to be too.
  const day = (back: number) =>
    new Date(Date.now() - back * 86_400_000).toISOString().slice(0, 10)
  const from = day(10)
  const to = day(0)
  renderApp(`${PATH}?range=custom&from=${from}&to=${to}`)
  expect(await screen.findByTestId("digest-days")).toBeInTheDocument()
  expect(end).toBe(
    new Date(Date.parse(to) + 86_400_000).toISOString().slice(0, 10)
  )
})

it("reads the nested decision detail without inventing commit dates", async () => {
  const decision = state.db.digest.details.get("dec_1")!
  server.use(
    http.get(`${API}/decisions/:id`, () =>
      HttpResponse.json({
        decision,
        session: null,
        cards: [],
        commitsError: null,
        commits: [
          {
            sha: "abc1234",
            subject: "real API commit",
            body: "",
            files: [],
            diff: "",
          },
        ],
      })
    )
  )
  renderApp(`${PATH}?decision=dec_1`)
  expect(await screen.findByTestId("digest-drawer-commits")).toHaveTextContent(
    "real API commit"
  )
})

it("updates search on URL navigation", async () => {
  const { router } = renderApp(`${PATH}?q=election`)
  await screen.findByTestId("digest-days")
  await act(() =>
    router.navigate({
      to: "/w/$ws/digest",
      params: { ws: WS },
      search: { q: "ring" },
    })
  )
  await waitFor(() =>
    expect(screen.getByTestId("digest-search")).toHaveValue("ring")
  )
  expect(await screen.findByTestId("digest-session-sess_a")).toBeInTheDocument()
})
