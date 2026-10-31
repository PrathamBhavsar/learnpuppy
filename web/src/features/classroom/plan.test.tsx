import { render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import type { ClassroomView } from "@/api/pending-types"
import { server } from "@/test/msw/server"
import { renderApp, renderWithProviders, resetUi } from "@/test/render"

import { PhaseStepper } from "./PhaseStepper"
import { layoutPlan } from "./plan-layout"
import { fitTransform } from "./use-pan-zoom"

const WS = "ws_rustasync"

beforeEach(resetUi)

type Plan = NonNullable<ClassroomView["plan"]>

const PLAN: Plan = {
  nodes: [
    { id: "a", title: "Futures", summary: "Lazy values.", status: "visited" },
    {
      id: "b",
      title: "Wakers",
      summary: "How polling resumes.",
      status: "current",
    },
    { id: "c", title: "Pinning", summary: "No moves.", status: "upcoming" },
    { id: "d", title: "Executors", summary: "Own tasks.", status: "upcoming" },
  ],
  edges: [
    { from: "a", to: "b" },
    { from: "b", to: "c" },
    { from: "b", to: "d" },
  ],
  currentId: "b",
}

describe("layoutPlan", () => {
  it("layers a DAG top to bottom and orders nodes topologically", () => {
    const layout = layoutPlan(PLAN, "TB")
    const at = (id: string) => layout.nodes.find((n) => n.id === id)
    expect(at("a")?.y).toBeLessThan(at("b")?.y ?? 0)
    expect(at("b")?.y).toBeLessThan(at("c")?.y ?? 0)
    expect(at("c")?.y).toBe(at("d")?.y)
    expect(layout.nodes.map((n) => n.order)).toEqual([0, 1, 2, 3])
    expect(layout.nodes[0]?.id).toBe("a")
    expect(layout.edges).toHaveLength(3)
    expect(layout.edges[0]?.d).toMatch(/^M[\d.]+ [\d.]+ L/)
  })

  it("lays out left to right for wide panels", () => {
    const layout = layoutPlan(PLAN, "LR")
    const at = (id: string) => layout.nodes.find((n) => n.id === id)
    expect(at("a")?.x).toBeLessThan(at("b")?.x ?? 0)
    expect(layout.width).toBeGreaterThan(layout.height)
  })

  it("ignores edges that point at unknown nodes", () => {
    const layout = layoutPlan(
      { ...PLAN, edges: [...PLAN.edges, { from: "a", to: "ghost" }] },
      "TB"
    )
    expect(layout.edges).toHaveLength(3)
  })
})

describe("fitTransform", () => {
  it("shrinks to fit and centres, never enlarging beyond 1", () => {
    expect(
      fitTransform({ width: 200, height: 100 }, { width: 100, height: 100 })
    ).toEqual({
      k: 0.5,
      x: 0,
      y: 25,
    })
    expect(
      fitTransform({ width: 50, height: 50 }, { width: 100, height: 100 })
    ).toEqual({
      k: 1,
      x: 25,
      y: 25,
    })
    expect(
      fitTransform({ width: 50, height: 50 }, { width: 0, height: 0 })
    ).toEqual({
      k: 1,
      x: 0,
      y: 0,
    })
  })
})

describe("PhaseStepper", () => {
  it("marks finished, active and upcoming phases", () => {
    render(
      <PhaseStepper kind="teach" phase="plan" probe={{ asked: 8, max: 8 }} />
    )
    expect(screen.getByTestId("phase-probe")).toHaveAttribute(
      "data-state",
      "done"
    )
    expect(screen.getByTestId("phase-plan")).toHaveAttribute(
      "data-state",
      "active"
    )
    expect(screen.getByTestId("phase-teach")).toHaveAttribute(
      "data-state",
      "todo"
    )
    expect(screen.getByTestId("phase-plan")).toHaveAttribute(
      "aria-current",
      "step"
    )
  })

  it("names the third step Practice for practice chats and shows probe progress while probing", () => {
    const { rerender } = render(
      <PhaseStepper
        kind="practice"
        phase="practice"
        probe={{ asked: 0, max: 8 }}
      />
    )
    expect(screen.getByTestId("phase-practice")).toHaveAttribute(
      "data-state",
      "active"
    )
    rerender(
      <PhaseStepper kind="teach" phase="probe" probe={{ asked: 3, max: 8 }} />
    )
    expect(screen.getByTestId("probe-progress")).toHaveAttribute(
      "aria-valuenow",
      "3"
    )
    expect(screen.getAllByTestId("probe-segment-filled")).toHaveLength(3)
  })

  it("completes every step when the chat is done", () => {
    render(
      <PhaseStepper kind="teach" phase="done" probe={{ asked: 8, max: 8 }} />
    )
    for (const step of ["probe", "plan", "teach"])
      expect(screen.getByTestId(`phase-${step}`)).toHaveAttribute(
        "data-state",
        "done"
      )
  })
})

describe("PlanDag in the classroom", () => {
  it("draws the plan with server statuses and explains a node on click", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/learn/c_teach_quiz`)
    const dag = await screen.findByTestId("plan-dag")
    expect(within(dag).getByTestId("plan-node-p1")).toHaveAttribute(
      "data-status",
      "visited"
    )
    expect(within(dag).getByTestId("plan-node-p2")).toHaveAttribute(
      "data-status",
      "current"
    )
    expect(within(dag).getByTestId("plan-node-p3")).toHaveAttribute(
      "data-status",
      "upcoming"
    )
    expect(dag).toHaveAttribute("dir", "ltr")
    await user.click(within(dag).getByTestId("plan-node-p2"))
    expect(await screen.findByTestId("plan-popover-p2")).toHaveTextContent(
      "poll returns Pending after storing the waker."
    )
    await user.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByTestId("plan-popover-p2")).not.toBeInTheDocument()
    )
  })

  it("opens a node from the keyboard", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/learn/c_teach_quiz`)
    const node = await screen.findByTestId("plan-node-p4")
    node.focus()
    await user.keyboard("{Enter}")
    expect(await screen.findByTestId("plan-popover-p4")).toHaveTextContent(
      "An executor owns tasks"
    )
  })

  it("zooms and refits", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/learn/c_teach_quiz`)
    await screen.findByTestId("plan-dag")
    expect(screen.getByTestId("plan-fit")).toBeDisabled()
    await user.click(screen.getByTestId("plan-zoom-in"))
    expect(screen.getByTestId("plan-fit")).toBeEnabled()
    await user.click(screen.getByTestId("plan-fit"))
    expect(screen.getByTestId("plan-fit")).toBeDisabled()
  })

  it("explains the missing plan while probing", async () => {
    renderApp(`/w/${WS}/learn/c_probe_pending`)
    expect(await screen.findByTestId("plan-empty")).toHaveTextContent(
      "plan appears once the probe questions are done"
    )
  })
})

describe("JobsPanel and ArtifactViewer", () => {
  async function openJobs(chat: string) {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/learn/${chat}`)
    await user.click(await screen.findByTestId("tab-jobs"))
    return user
  }

  it("lists jobs with their status and opens Markdown results in a sheet", async () => {
    const user = await openJobs("c_teach_quiz")
    expect(await screen.findByTestId("job-job_research_done")).toHaveAttribute(
      "data-status",
      "done"
    )
    expect(screen.getByTestId("job-job_pdf_failed")).toHaveAttribute(
      "data-status",
      "failed"
    )
    await user.click(screen.getByTestId("job-open-job_research_done"))
    const body = await screen.findByTestId("artifact-markdown")
    await waitFor(() =>
      expect(body).toHaveTextContent("Cancel-safe APIs in tokio")
    )
    await waitFor(() => expect(body.querySelector("table")).not.toBeNull())
  })

  it("renders HTML results in an iframe with an empty sandbox", async () => {
    const user = await openJobs("c_teach_quiz")
    await user.click(await screen.findByTestId("job-open-job_visual_done"))
    const frame = await screen.findByTestId("artifact-html")
    expect(frame.tagName).toBe("IFRAME")
    expect(frame).toHaveAttribute("sandbox", "")
    expect(frame.getAttribute("srcdoc")).toContain("Poll loop")
    expect(frame).not.toHaveAttribute("src")
  })

  it("shows an error with retry when the artifact cannot be loaded", async () => {
    server.use(
      http.get("*/api/v1/workspaces/:ws/artifacts/:id", () =>
        HttpResponse.json(
          {
            error: {
              code: "internal",
              message: "Artifact unreadable",
              retryable: false,
              requestId: "req_x",
            },
          },
          { status: 500 }
        )
      )
    )
    const user = await openJobs("c_teach_quiz")
    await user.click(await screen.findByTestId("job-open-job_research_done"))
    expect(await screen.findByText("Artifact unreadable")).toBeInTheDocument()
    expect(screen.getByText(/req_x/)).toBeInTheDocument()
  })

  it("aborts a running job", async () => {
    let aborted = ""
    server.use(
      http.post("*/api/v1/workspaces/:ws/jobs/:id/abort", ({ params }) => {
        aborted = String(params["id"])
        return HttpResponse.json({
          id: aborted,
          chatId: "c_running",
          kind: "research",
          status: "failed",
          title: "Research",
          artifactId: null,
          createdAt: 1,
        })
      })
    )
    const user = await openJobs("c_running")
    expect(await screen.findByTestId("job-job_research_1")).toHaveAttribute(
      "data-status",
      "running"
    )
    await user.click(screen.getByTestId("job-abort-job_research_1"))
    await waitFor(() => expect(aborted).toBe("job_research_1"))
  })

  it("explains an empty job list", async () => {
    await openJobs("c_done")
    expect(await screen.findByTestId("jobs-empty")).toBeInTheDocument()
  })
})

describe("RateLimitBanner", () => {
  it("ticks down every second", async () => {
    const { RateLimitBanner } = await import("./TurnStatus")
    const retryAt = Date.now() + 5000
    renderWithProviders(
      <RateLimitBanner
        turn={{ state: "queued", retryAt }}
        onCancel={() => undefined}
        cancelling={false}
      />
    )
    const first = Number.parseInt(
      screen.getByTestId("rate-limit-countdown").textContent ?? "",
      10
    )
    await waitFor(
      () =>
        expect(
          Number.parseInt(
            screen.getByTestId("rate-limit-countdown").textContent ?? "",
            10
          )
        ).toBeLessThan(first),
      { timeout: 3000 }
    )
  })
})
