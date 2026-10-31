import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { FakeEventSource } from "@/test/fake-event-source"
import { state } from "@/test/msw/db"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

import {
  actionFor,
  digestPath,
  normalizePath,
  percent,
  sinceOptionsFor,
  sourcesEqual,
  validatePath,
} from "./model"

beforeEach(resetUi)

const WS = "ws_rustasync"
const OTHER = "ws_distsys"
const API = "*/api/v1"

async function open() {
  const view = renderApp(`/w/${WS}/ingest`)
  await screen.findByTestId("screen-ingest")
  return { ...view, user: userEvent.setup() }
}

async function stream() {
  const source = await waitFor(() => {
    const found = [...FakeEventSource.instances]
      .reverse()
      .find((s) => s.workspace === WS)
    if (!found) throw new Error("no workspace stream yet")
    return found
  })
  source.open()
  let id = 1
  return (type: string, data: unknown) => source.emit(type, data, id++)
}

describe("model", () => {
  it("validates paths on shape only", () => {
    expect(validatePath("", [])).toMatch(/Enter/)
    expect(validatePath("relative/x", [])).toMatch(/absolute/)
    expect(validatePath("/a/b", ["/a/b"])).toMatch(/Already/)
    expect(validatePath("/a/b/", ["/a/b"])).toMatch(/Already/)
    expect(validatePath("~/code", [])).toBeNull()
    expect(normalizePath(" /a/b// ")).toBe("/a/b")
    expect(normalizePath("/")).toBe("/")
  })

  it("compares sources, adds a custom since option and clamps progress", () => {
    const a = {
      claude: true,
      codex: true,
      agy: true,
      git: true,
      allow: ["/a"],
      sinceDays: 30,
    }
    expect(sourcesEqual(a, { ...a, allow: ["/a"] })).toBe(true)
    expect(sourcesEqual(a, { ...a, allow: ["/a", "/b"] })).toBe(false)
    expect(sourcesEqual(a, { ...a, git: false })).toBe(false)
    expect(sinceOptionsFor(45).at(-1)).toEqual({ days: 45, label: "45 days" })
    expect(sinceOptionsFor(30)).toHaveLength(4)
    expect(percent(3, 12)).toBe(25)
    expect(percent(5, 0)).toBe(0)
    expect(percent(99, 12)).toBe(100)
  })

  it("maps known failures to an actionable hint", () => {
    expect(
      actionFor("agy: conversation_summaries.db is locked (snapshot failed)")
    ).toMatch(/Close Antigravity/)
    expect(actionFor("claude: /x is not readable (permission denied)")).toMatch(
      /read access/
    )
    expect(actionFor("something odd")).toBeNull()
  })

  it("builds the digest link from the run window", () => {
    const start = new Date(2026, 9, 5, 10).getTime()
    const path = digestPath("ws_1", {
      id: "r",
      startedAt: start,
      finishedAt: start + 60_000,
      report: {
        sessions: 0,
        newDecisions: 1,
        newCards: 0,
        skipped: 0,
        errors: [],
      },
    })
    expect(path).toBe("/w/ws_1/digest?from=2026-10-05&to=2026-10-05")
  })
})

describe("Ingest: five states", () => {
  it("loading then populated", async () => {
    renderApp(`/w/${WS}/ingest`)
    expect(await screen.findByTestId("ingest-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("ingest-sources")).toBeInTheDocument()
    expect(screen.getByTestId("allow-item-0")).toHaveTextContent(
      "/Users/dev/code/tokio-notes"
    )
    expect(screen.getByTestId("source-toggle-claude")).toBeChecked()
    expect(screen.getByTestId("since-30")).toHaveAttribute("data-state", "on")
  })

  it("error: surfaces the server message and retries", async () => {
    let calls = 0
    server.use(
      http.get(`${API}/workspaces/:ws/ingest/sources`, () => {
        calls += 1
        return calls === 1
          ? apiError("internal", "Ledger unreadable")
          : HttpResponse.json(state.db.ingest.get(WS)?.sources)
      })
    )
    const { user } = await open()
    expect(await screen.findByTestId("error-state")).toHaveTextContent(
      "Ledger unreadable"
    )
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByTestId("ingest-sources")).toBeInTheDocument()
  })

  it("empty allow-list: explains itself and keeps Run disabled", async () => {
    const entry = state.db.ingest.get(WS)
    if (!entry) throw new Error("fixture missing")
    entry.sources.sources.allow = []
    entry.sources.enabled = false
    await open()
    expect(await screen.findByTestId("allow-empty")).toHaveTextContent(
      "No folders yet"
    )
    expect(screen.getByTestId("empty-state")).toHaveTextContent(
      "Choose where to read from"
    )
    expect(screen.getByTestId("run-start")).toBeDisabled()
    expect(screen.getByTestId("run-blocked")).toHaveTextContent(
      "Add a project folder"
    )
  })

  it("empty history shows a hint instead of a table", async () => {
    const entry = state.db.ingest.get(WS)
    if (!entry) throw new Error("fixture missing")
    entry.runs = []
    await open()
    expect(await screen.findByTestId("history-empty")).toBeInTheDocument()
    expect(screen.queryByTestId("last-run")).not.toBeInTheDocument()
  })
})

describe("Ingest: sources", () => {
  it("toggling a tool or the window makes the draft dirty, and Save persists it", async () => {
    const { user } = await open()
    await screen.findByTestId("ingest-sources")
    expect(screen.getByTestId("sources-save")).toBeDisabled()
    await user.click(screen.getByTestId("source-toggle-git"))
    await user.click(screen.getByTestId("since-90"))
    expect(screen.getByTestId("sources-save")).toBeEnabled()
    expect(screen.getByTestId("run-start")).toBeDisabled()
    await user.click(screen.getByTestId("sources-save"))
    await waitFor(() =>
      expect(state.db.ingest.get(WS)?.sources.sources.git).toBe(false)
    )
    expect(state.db.ingest.get(WS)?.sources.sources.sinceDays).toBe(90)
    await waitFor(() =>
      expect(screen.getByTestId("sources-save")).toBeDisabled()
    )
    expect(screen.getByTestId("run-start")).toBeEnabled()
  })

  it("Reset throws the edits away", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("source-toggle-codex"))
    await user.click(screen.getByTestId("sources-reset"))
    expect(screen.getByTestId("source-toggle-codex")).toBeChecked()
  })

  it("manual path: client check, add, remove", async () => {
    const { user } = await open()
    const input = await screen.findByTestId("manual-path")
    await user.type(input, "relative")
    await user.click(screen.getByTestId("manual-add"))
    expect(screen.getByTestId("manual-path-error")).toHaveTextContent(
      "absolute"
    )
    await user.clear(input)
    await user.type(input, "/Users/dev/code/new-thing{Enter}")
    expect(screen.getByTestId("allow-item-2")).toHaveTextContent(
      "/Users/dev/code/new-thing"
    )
    await user.click(screen.getByTestId("allow-remove-2"))
    expect(screen.queryByTestId("allow-item-2")).not.toBeInTheDocument()
  })

  it("Detect projects lists candidates with counts and adds the ticked ones", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("detect-projects"))
    const dialog = await screen.findByTestId("detect-dialog")
    expect(
      within(dialog).getByTestId("candidate-/Users/dev/code/raft-lab")
    ).toHaveTextContent("14 sessions")
    expect(within(dialog).getByTestId("detect-add")).toBeDisabled()
    await user.click(within(dialog).getByRole("checkbox", { name: /raft-lab/ }))
    await user.click(within(dialog).getByTestId("detect-add"))
    await waitFor(() =>
      expect(screen.queryByTestId("detect-dialog")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("allow-list")).toHaveTextContent(
      "/Users/dev/code/raft-lab"
    )
  })

  it("an unreadable folder is reported on its own chip", async () => {
    const { user } = await open()
    await user.type(
      await screen.findByTestId("manual-path"),
      "/Users/dev/unreadable{Enter}"
    )
    await user.click(screen.getByTestId("sources-save"))
    expect(await screen.findByTestId("allow-error-2")).toHaveTextContent(
      "not readable"
    )
  })

  it("warns when a folder overlaps another workspace", async () => {
    const other = state.db.ingest.get(OTHER)
    if (!other) throw new Error("fixture missing")
    other.sources.sources.allow = ["/Users/dev/code/raft-lab"]
    const { user } = await open()
    await user.type(
      await screen.findByTestId("manual-path"),
      "/Users/dev/code/raft-lab{Enter}"
    )
    await user.click(screen.getByTestId("sources-save"))
    const warning = await screen.findByTestId("overlap-warning")
    expect(warning).toHaveTextContent("/Users/dev/code/raft-lab")
    expect(warning).toHaveTextContent("another workspace")
  })
})

describe("Ingest: running", () => {
  it("shows live progress from the event stream, then the finished summary", async () => {
    const { user } = await open()
    const emit = await stream()
    await user.click(await screen.findByTestId("run-start"))
    expect(await screen.findByTestId("run-progress")).toBeInTheDocument()
    const runId = [...state.db.ingest.get(WS)!.runs][0]!.id

    act(() =>
      emit("ingest.progress", {
        runId,
        done: 6,
        total: 12,
        phase: "extracting",
      })
    )
    await waitFor(() =>
      expect(screen.getByTestId("run-phase")).toHaveTextContent(
        "Extracting decisions"
      )
    )
    expect(screen.getByTestId("run-bar")).toHaveAttribute("aria-valuenow", "50")
    expect(screen.getByTestId("ingest-run-panel")).toHaveAttribute(
      "data-state",
      "running"
    )

    await waitFor(
      () => expect(screen.getByTestId("last-run")).toBeInTheDocument(),
      { timeout: 4000 }
    )
    expect(screen.getByTestId("run-start")).toBeEnabled()
  })

  it("Cancel stops the run and the history records it", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("run-start"))
    await user.click(await screen.findByTestId("run-cancel"))
    await waitFor(() =>
      expect(screen.queryByTestId("run-progress")).not.toBeInTheDocument()
    )
    const runs = state.db.ingest.get(WS)!.runs
    expect(runs[0]!.report.errors[0]).toMatch(/Cancelled/)
    expect(runs[0]!.finishedAt).not.toBeNull()
  })

  it("a second start while one is running surfaces the conflict", async () => {
    server.use(
      http.post(`${API}/workspaces/:ws/ingest/run`, () =>
        apiError("conflict", "An ingest run is already in progress")
      )
    )
    const { user } = await open()
    await user.click(await screen.findByTestId("run-start"))
    expect(
      await screen.findByText("Could not start ingest")
    ).toBeInTheDocument()
  })
})

describe("Ingest: history and errors", () => {
  it("lists runs; expanding one shows the report, its errors and an actionable hint", async () => {
    const { user } = await open()
    const row = await screen.findByTestId("run-run_2")
    expect(within(row).getByTestId("run-errors-run_2")).toHaveTextContent("2")
    await user.click(screen.getByTestId("run-toggle-run_2"))
    const details = await screen.findByTestId("run-details-run_2")
    expect(details).toHaveTextContent("conversation_summaries.db is locked")
    expect(details).toHaveTextContent("Close Antigravity")
    expect(details).toHaveTextContent("Grant read access")
  })

  it("a run with new decisions links into the digest for its dates", async () => {
    const { user, router } = await open()
    await user.click(await screen.findByTestId("run-toggle-run_3"))
    await user.click(await screen.findByTestId("run-digest-run_3"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${WS}/digest`)
    )
    expect(router.state.location.search).toMatchObject({
      from: expect.any(String),
      to: expect.any(String),
    })
  })

  it("the last-run summary counts up the report", async () => {
    await open()
    const last = await screen.findByTestId("last-run")
    expect(last).toHaveTextContent("Sessions")
    expect(last).toHaveTextContent("New decisions")
  })
})
