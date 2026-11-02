import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

const WS = "ws_rustasync"
const PATH = `/w/${WS}/digest`
const API = `*/api/v1/workspaces/${WS}/digest`

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

const days = () => state.db.digest.days

describe("Digest states", () => {
  it("shows a skeleton, then day groups with counts and session cards", async () => {
    renderApp(PATH)
    const section = await screen.findByTestId("screen-digest")
    expect(section).toHaveAttribute("aria-busy", "true")
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument()

    const list = await screen.findByTestId("digest-days")
    expect(section).toHaveAttribute("aria-busy", "false")
    const today = within(list).getByTestId(`digest-day-${days()[0]!.day}`)
    expect(within(today).getByText("Today")).toBeInTheDocument()
    expect(
      within(today).getByTestId(`digest-day-count-${days()[0]!.day}`)
    ).toHaveTextContent("2 sessions · 3 decisions")
    expect(within(list).getByText("Yesterday")).toBeInTheDocument()
    expect(within(list).getByTestId("digest-session-sess_a")).toHaveTextContent(
      "learnpuppy"
    )
    expect(within(list).getByTestId("digest-session-sess_a")).toHaveTextContent(
      "2 decisions"
    )
    expect(within(list).getByTestId("digest-session-sess_a")).toHaveTextContent(
      "2 cards"
    )
  })

  it("explains ingest when there is nothing at all", async () => {
    server.use(http.get(API, () => HttpResponse.json([])))
    renderApp(PATH)
    expect(await screen.findByText("Nothing to digest yet")).toBeInTheDocument()
    expect(screen.getByTestId("digest-go-ingest")).toHaveAttribute(
      "href",
      `/w/${WS}/ingest`
    )
    expect(screen.queryByTestId("digest-review-these")).not.toBeInTheDocument()
  })

  it("shows an error with the request id and retries", async () => {
    let fail = true
    server.use(
      http.get(API, () =>
        fail
          ? HttpResponse.json(
              {
                error: {
                  code: "internal",
                  message: "boom",
                  requestId: "req_d1",
                },
              },
              { status: 500 }
            )
          : HttpResponse.json([])
      )
    )
    const user = userEvent.setup()
    renderApp(PATH)
    expect(await screen.findByTestId("error-state")).toHaveTextContent("req_d1")
    fail = false
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByText("Nothing to digest yet")).toBeInTheDocument()
  })

  it("keeps showing loaded days with a warning when a refresh fails", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await screen.findByTestId("digest-days")
    server.use(
      http.get(API, () =>
        HttpResponse.json(
          { error: { code: "internal", message: "later", requestId: "req_s" } },
          { status: 500 }
        )
      )
    )
    // Changing a filter and changing it back reuses the cached days while the refetch fails.
    await user.click(screen.getByTestId("digest-range-30d"))
    await user.click(screen.getByTestId("digest-range-7d"))
    expect(await screen.findByTestId("digest-stale")).toBeInTheDocument()
    expect(screen.getByTestId("digest-days")).toBeInTheDocument()
  })
})

describe("Digest filters (URL-backed)", () => {
  it("filters by source chip", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(PATH)
    await screen.findByTestId("digest-days")
    await user.click(screen.getByTestId("digest-source-codex"))
    await waitFor(() => {
      expect(
        screen.queryByTestId("digest-session-sess_a")
      ).not.toBeInTheDocument()
    })
    expect(screen.getByTestId("digest-session-sess_b")).toBeInTheDocument()
    expect(screen.getByTestId("digest-source-codex")).toHaveAttribute(
      "aria-pressed",
      "true"
    )
    expect(router.state.location.search).toMatchObject({ src: "codex" })
  })

  it("narrows to today with the range preset", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(PATH)
    await screen.findByTestId("digest-days")
    await user.click(screen.getByTestId("digest-range-today"))
    await waitFor(() =>
      expect(screen.queryByText("Yesterday")).not.toBeInTheDocument()
    )
    expect(
      within(screen.getByTestId("digest-days")).getByText("Today")
    ).toBeInTheDocument()
    expect(router.state.location.search).toMatchObject({ range: "today" })
  })

  it("offers a custom range with date inputs", async () => {
    const user = userEvent.setup()
    renderApp(`${PATH}?range=custom&from=2020-01-01&to=2020-01-02`)
    expect(await screen.findByTestId("digest-custom-range")).toBeInTheDocument()
    expect(screen.getByTestId("digest-from")).toHaveValue("2020-01-01")
    expect(await screen.findByText("No decisions match")).toBeInTheDocument()
    await user.click(screen.getByTestId("digest-clear-filters"))
    expect(await screen.findByTestId("digest-days")).toBeInTheDocument()
  })

  it("searches decisions, projects and summaries, and offers to clear", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(PATH)
    await screen.findByTestId("digest-days")
    await user.type(screen.getByTestId("digest-search"), "election")
    await waitFor(() =>
      expect(
        screen.queryByTestId("digest-session-sess_a")
      ).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("digest-session-sess_c")).toBeInTheDocument()
    await waitFor(() =>
      expect(router.state.location.search).toMatchObject({ q: "election" })
    )

    await user.clear(screen.getByTestId("digest-search"))
    await user.type(screen.getByTestId("digest-search"), "no such thing")
    expect(await screen.findByText("No decisions match")).toBeInTheDocument()
    await user.click(screen.getByTestId("digest-clear-filters"))
    expect(
      await screen.findByTestId("digest-session-sess_a")
    ).toBeInTheDocument()
    expect(screen.getByTestId("digest-search")).toHaveValue("")
  })

  it("restores filters from the URL", async () => {
    renderApp(`${PATH}?src=agy&q=heartbeat`)
    expect(
      await screen.findByTestId("digest-session-sess_c")
    ).toBeInTheDocument()
    expect(screen.getByTestId("digest-source-agy")).toHaveAttribute(
      "aria-pressed",
      "true"
    )
    expect(screen.getByTestId("digest-search")).toHaveValue("heartbeat")
    expect(
      screen.queryByTestId("digest-session-sess_a")
    ).not.toBeInTheDocument()
  })

  it("collapses and expands a day group", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await screen.findByTestId("digest-days")
    const day = days()[0]!.day
    const toggle = screen.getByTestId(`digest-day-toggle-${day}`)
    expect(toggle).toHaveAttribute("aria-expanded", "true")
    await user.click(toggle)
    await waitFor(() =>
      expect(
        screen.queryByTestId("digest-session-sess_a")
      ).not.toBeInTheDocument()
    )
    expect(toggle).toHaveAttribute("aria-expanded", "false")
    await user.click(toggle)
    expect(
      await screen.findByTestId("digest-session-sess_a")
    ).toBeInTheDocument()
  })
})

describe("Session cards and decision drawer", () => {
  it("expands a session to its decisions and opens the drawer", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(PATH)
    await screen.findByTestId("digest-days")
    await user.click(screen.getByTestId("digest-session-toggle-sess_a"))
    await user.click(await screen.findByTestId("digest-decision-dec_1"))

    const drawer = await screen.findByTestId("digest-drawer")
    expect(
      await within(drawer).findByTestId("digest-drawer-title")
    ).toHaveTextContent("Use a ring buffer for event replay")
    expect(within(drawer).getByTestId("digest-drawer-basis")).toHaveTextContent(
      "Stated"
    )
    expect(
      within(drawer).getByTestId("digest-drawer-chosen")
    ).toHaveTextContent("4096-entry ring")
    const alternatives = within(drawer).getByTestId(
      "digest-drawer-alternatives"
    )
    expect(within(alternatives).getAllByRole("listitem")).toHaveLength(2)
    expect(alternatives).toHaveTextContent("Rejected because")
    expect(
      within(drawer).getByTestId("digest-drawer-commits")
    ).toHaveTextContent("4643f9d")
    expect(router.state.location.search).toMatchObject({ decision: "dec_1" })

    const cardLink = await within(drawer).findByTestId(
      "digest-drawer-card-card_rustasync_001"
    )
    expect(cardLink.getAttribute("href")).toContain("tab=cards")
  })

  it("renders math in the decision text", async () => {
    const detail = state.db.digest.details.get("dec_1")
    if (!detail) throw new Error("fixture dec_1 missing")
    state.db.digest.details.set("dec_1", {
      ...detail,
      chosen: String.raw`Keep \(x^2\) bounded`,
      why: String.raw`Because $\sum_{i=1}^{n} i$ grows`,
    })
    renderApp(`${PATH}?decision=dec_1`)
    const drawer = await screen.findByTestId("digest-drawer")
    await waitFor(() =>
      expect(
        drawer.querySelector('[data-testid="digest-drawer-chosen"] .katex')
      ).not.toBeNull()
    )
    await waitFor(() =>
      expect(drawer.querySelectorAll(".katex").length).toBeGreaterThanOrEqual(2)
    )
  })

  it("opens straight from the URL and closes back to the list", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(`${PATH}?decision=dec_3`)
    const drawer = await screen.findByTestId("digest-drawer")
    expect(
      await within(drawer).findByTestId("digest-drawer-basis")
    ).toHaveTextContent("Inferred")
    await user.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByTestId("digest-drawer")).not.toBeInTheDocument()
    )
    expect(router.state.location.search).not.toHaveProperty("decision")
  })

  it("shows an error inside the drawer for an unknown decision", async () => {
    renderApp(`${PATH}?decision=dec_missing`)
    const drawer = await screen.findByTestId("digest-drawer")
    expect(await within(drawer).findByTestId("error-state")).toBeInTheDocument()
  })

  it("regenerates cards and confirms with a toast", async () => {
    const user = userEvent.setup()
    renderApp(`${PATH}?decision=dec_1`)
    const drawer = await screen.findByTestId("digest-drawer")
    await user.click(await within(drawer).findByTestId("digest-regenerate"))
    expect(await screen.findByText("Regenerated 1 card")).toBeInTheDocument()
    expect(
      within(drawer).queryByTestId("digest-regenerate-errors")
    ).not.toBeInTheDocument()
  })

  it("lists validation errors when regeneration is rejected", async () => {
    state.db.digest.details.get("dec_3")!.alternatives = []
    const user = userEvent.setup()
    renderApp(`${PATH}?decision=dec_3`)
    const drawer = await screen.findByTestId("digest-drawer")
    await user.click(await within(drawer).findByTestId("digest-regenerate"))
    const errors = await within(drawer).findByTestId("digest-regenerate-errors")
    expect(errors).toHaveTextContent("alternatives")
    expect(errors).toHaveTextContent("A card needs at least three distractors")
    // Not a toast: field errors stay in the drawer.
    expect(
      screen.queryByText("Could not regenerate cards")
    ).not.toBeInTheDocument()
  })

  it("lists drafts the server rejected after a partial save", async () => {
    server.use(
      http.post(`${API}/decisions/:id/cards`, () =>
        HttpResponse.json({
          cardIds: [],
          saved: 0,
          rejected: [{ front: "Why use a ring?", reason: "Only two options" }],
        })
      )
    )
    const user = userEvent.setup()
    renderApp(`${PATH}?decision=dec_1`)
    const drawer = await screen.findByTestId("digest-drawer")
    await user.click(await within(drawer).findByTestId("digest-regenerate"))
    expect(
      await within(drawer).findByTestId("digest-regenerate-errors")
    ).toHaveTextContent("Only two options")
  })

  it("disables regenerate while a request is running", async () => {
    server.use(
      http.post(`${API}/decisions/:id/cards`, async () => {
        await new Promise((r) => setTimeout(r, 200))
        return HttpResponse.json({ cardIds: ["card_rustasync_001"] })
      })
    )
    const user = userEvent.setup()
    renderApp(`${PATH}?decision=dec_1`)
    const drawer = await screen.findByTestId("digest-drawer")
    const button = await within(drawer).findByTestId("digest-regenerate")
    await user.click(button)
    expect(button).toBeDisabled()
    expect(button).toHaveTextContent("Regenerating…")
    await waitFor(() => expect(button).toBeEnabled())
  })
})

describe("Review these decisions", () => {
  it("links to a review session filtered to the visible decisions that have cards", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    const cta = await screen.findByTestId("digest-review-these")
    expect(cta).toHaveTextContent("Review these 6 decisions")
    const href = decodeURIComponent(cta.getAttribute("href") ?? "")
    expect(href).toContain(`/w/${WS}/review`)
    expect(href).toContain("decisions=dec_1,dec_2,dec_3,dec_4,dec_5,dec_6")

    await user.click(screen.getByTestId("digest-source-codex"))
    await waitFor(() =>
      expect(screen.getByTestId("digest-review-these")).toHaveTextContent(
        "Review these 1 decision"
      )
    )
    await user.click(screen.getByTestId("digest-review-these"))
    expect(await screen.findByTestId("screen-review")).toBeInTheDocument()
    expect(await screen.findByTestId("review-start")).toHaveTextContent(
      "Start review (1)"
    )
  })
})
