import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { state } from "@/test/msw/db"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

beforeEach(resetUi)

const API = "*/api/v1"
const WS = "ws_rustasync"

async function open(path = "/settings") {
  const view = renderApp(path)
  await screen.findByTestId("screen-settings")
  return { ...view, user: userEvent.setup() }
}

describe("Settings: five states", () => {
  it("loading then populated", async () => {
    renderApp("/settings")
    expect(await screen.findByTestId("settings-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("settings-appearance")).toBeInTheDocument()
    expect(screen.getByTestId("about-version")).toBeInTheDocument()
  })

  it("error: message and retry", async () => {
    let calls = 0
    server.use(
      http.get(`${API}/settings`, () => {
        calls += 1
        return calls === 1
          ? apiError("internal", "Settings file is corrupt")
          : HttpResponse.json(state.db.settings)
      })
    )
    const { user } = await open()
    expect(await screen.findByTestId("error-state")).toHaveTextContent(
      "Settings file is corrupt"
    )
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByTestId("settings-appearance")).toBeInTheDocument()
  })
})

describe("Settings: appearance", () => {
  it("theme changes the document class, the store and the saved document", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("theme-dark"))
    expect(document.documentElement).toHaveClass("dark")
    expect(useUi.getState().theme).toBe("dark")
    expect(localStorage.getItem("lp-theme")).toBe("dark")
    await waitFor(() => expect(state.db.settings.ui.theme).toBe("dark"))
    await user.click(screen.getByTestId("theme-light"))
    expect(document.documentElement).toHaveClass("light")
  })

  it("direction mirrors the page and persists", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("direction-rtl"))
    expect(document.documentElement.dir).toBe("rtl")
    expect(localStorage.getItem("lp-dir")).toBe("rtl")
    await waitFor(() => expect(state.db.settings.ui.direction).toBe("rtl"))
    await user.click(screen.getByTestId("direction-ltr"))
    expect(document.documentElement.dir).toBe("ltr")
  })

  it("reduce motion override sets the root flag and persists", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("reduce-motion"))
    expect(document.documentElement.dataset["reduceMotion"]).toBe("true")
    expect(localStorage.getItem("lp-reduce-motion")).toBe("1")
    await waitFor(() => expect(state.db.settings.ui.reduceMotion).toBe(true))
    await user.click(screen.getByTestId("reduce-motion"))
    expect(document.documentElement.dataset["reduceMotion"]).toBeUndefined()
  })

  it("density compact tightens the root and persists", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("density-compact"))
    expect(document.documentElement.dataset["density"]).toBe("compact")
    expect(localStorage.getItem("lp-density")).toBe("compact")
    await waitFor(() => expect(state.db.settings.ui.density).toBe("compact"))
    await user.click(screen.getByTestId("density-comfortable"))
    expect(document.documentElement.dataset["density"]).toBeUndefined()
  })
})

describe("Settings: behavior", () => {
  it("toggles are saved to the settings document", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("confirm-destructive"))
    await waitFor(() =>
      expect(state.db.settings.ui.confirmDestructive).toBe(false)
    )
    await user.click(screen.getByTestId("auto-open"))
    await waitFor(() => expect(state.db.settings.ui.autoOpenBrowser).toBe(true))
    // The earlier change must survive the later one.
    expect(state.db.settings.ui.confirmDestructive).toBe(false)
  })

  it("the concurrency cap commits on release and stays within 1..6", async () => {
    const { user } = await open()
    const slider = await screen.findByRole("slider", {
      name: "Concurrency cap",
    })
    expect(slider).toHaveAttribute("aria-valuemin", "1")
    expect(slider).toHaveAttribute("aria-valuemax", "6")
    expect(screen.getByTestId("concurrency-value")).toHaveTextContent("2")
    slider.focus()
    await user.keyboard("{ArrowRight}{ArrowRight}")
    await waitFor(() =>
      expect(screen.getByTestId("concurrency-value")).toHaveTextContent("4")
    )
    await waitFor(() => expect(state.db.settings.concurrencyCap).toBe(4))
  })

  it("a failed save toasts instead of failing silently", async () => {
    server.use(
      http.put(`${API}/settings`, () =>
        apiError("validation", "Cap out of range")
      )
    )
    const { user } = await open()
    await user.click(await screen.findByTestId("auto-open"))
    expect(
      await screen.findByText("Could not save this setting")
    ).toBeInTheDocument()
  })
})

describe("Settings: shortcuts, data and about", () => {
  it("lists the registered shortcuts from the hotkey registry", async () => {
    await open()
    const table = await screen.findByTestId("settings-shortcuts")
    expect(
      within(table).getByTestId("settings-shortcut-shortcuts")
    ).toHaveTextContent("Keyboard shortcuts")
  })

  it("shows the data directory and links to the trash", async () => {
    const { user, router } = await open()
    await waitFor(() =>
      expect(screen.getByTestId("data-dir")).toHaveTextContent(
        "/mock/.learnpuppy"
      )
    )
    await user.click(screen.getByTestId("open-trash"))
    await waitFor(() => expect(router.state.location.pathname).toBe("/trash"))
  })
})

describe("Settings: diagnostics", () => {
  const TOKEN = "sk-ant-oat01-PLANTEDSECRET1234567890abcdef"

  it("copies the report and shows what was copied", async () => {
    const { user } = await open()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    })
    await user.click(await screen.findByTestId("copy-diagnostics"))
    const shown = await screen.findByTestId("diagnostics-text")
    expect(writeText).toHaveBeenCalledWith(shown.textContent)
    expect(shown).toHaveTextContent("LearnPuppy diagnostics")
  })

  it("never copies a planted token, whether the server or the browser built the report", async () => {
    server.use(
      http.post(`${API}/diagnostics`, () =>
        HttpResponse.json({
          text: `version: x\naccess_token=${TOKEN}\nAuthorization: Bearer ${TOKEN}`,
        })
      )
    )
    const { user } = await open()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    })
    await user.click(await screen.findByTestId("copy-diagnostics"))
    await screen.findByTestId("diagnostics-text")
    expect(writeText.mock.calls[0]?.[0]).not.toContain("PLANTEDSECRET")
    expect(screen.getByTestId("diagnostics-text")).not.toHaveTextContent(
      "PLANTEDSECRET"
    )

    // Server route missing: the browser-built report, with a token planted in an account field.
    server.use(
      http.post(`${API}/diagnostics`, () => apiError("not_found", "no route"))
    )
    const account = state.db.accounts[0]
    if (!account) throw new Error("fixture missing")
    account.cooldownReason = TOKEN
    account.email = `user+${TOKEN}@example.com`
    await act(async () => {
      await user.click(screen.getByTestId("copy-diagnostics"))
    })
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(2))
    const second = String(writeText.mock.calls[1]?.[0])
    expect(second).toContain("accounts:")
    expect(second).not.toContain("PLANTEDSECRET")
    expect(second).not.toContain("example.com")
  })
})

describe("Workspace settings", () => {
  it("loading, then general form, sources link, overrides and danger zone", async () => {
    renderApp(`/w/${WS}/settings`)
    expect(await screen.findByTestId("settings-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("ws-general")).toBeInTheDocument()
    expect(screen.getByTestId("ws-name")).toHaveValue("Rust async")
    expect(screen.getByTestId("ws-sources-link")).toHaveAttribute(
      "href",
      `/w/${WS}/ingest`
    )
    expect(screen.getByTestId("section-danger")).toBeInTheDocument()
  })

  it("renames the workspace and rejects an empty name on the field", async () => {
    const { user } = await open(`/w/${WS}/settings`)
    const name = await screen.findByTestId("ws-name")
    await user.clear(name)
    await user.type(name, "Rust async v2")
    await user.click(screen.getByTestId("ws-save"))
    await waitFor(() =>
      expect(state.db.workspaces.get(WS)?.name).toBe("Rust async v2")
    )

    await waitFor(() =>
      expect(screen.getByTestId("ws-name")).toHaveValue("Rust async v2")
    )
    await user.clear(screen.getByTestId("ws-name"))
    await user.click(screen.getByTestId("ws-save"))
    expect(await screen.findByTestId("ws-name-error")).toHaveTextContent("name")
  })

  it("overrides start inherited; turning one on, editing and saving writes only that role", async () => {
    const { user } = await open(`/w/${WS}/settings`)
    const row = await screen.findByTestId("ws-role-teacher")
    expect(row).toHaveAttribute("data-locked", "true")
    await user.click(screen.getByTestId("ws-override-teacher"))
    expect(screen.getByTestId("ws-role-teacher")).toHaveAttribute(
      "data-locked",
      "false"
    )
    await user.click(screen.getByTestId("ws-model-teacher"))
    await user.click(
      await screen.findByRole("option", { name: "claude-haiku-5-5" })
    )
    await user.click(screen.getByTestId("ws-review"))
    expect(await screen.findByTestId("ws-diff-list")).toHaveTextContent(
      "Teacher"
    )
    await user.click(screen.getByTestId("ws-confirm"))
    await waitFor(() =>
      expect(
        Object.keys(state.db.workspaces.get(WS)?.overrides.profiles ?? {})
      ).toEqual(["teacher"])
    )
    expect(
      state.db.workspaces.get(WS)?.overrides.profiles["teacher"]?.model
    ).toBe("claude-haiku-5-5")
    expect(state.db.settings.profiles["teacher"]?.model).toBe("claude-opus-5-5")
  })

  it("delete needs the typed name", async () => {
    const { user, router } = await open(`/w/${WS}/settings`)
    await user.click(await screen.findByTestId("ws-delete"))
    const submit = await screen.findByTestId("ws-delete-submit")
    expect(submit).toBeDisabled()
    await user.type(screen.getByTestId("ws-delete-confirm"), "Rust async")
    expect(submit).toBeEnabled()
    await user.click(submit)
    await waitFor(() => expect(state.db.trash.length).toBe(1))
    await waitFor(() => expect(router.state.location.pathname).toBe("/"))
  })

  it("archiving returns to the home screen", async () => {
    const { user, router } = await open(`/w/${WS}/settings`)
    await user.click(await screen.findByTestId("ws-archive"))
    await waitFor(() =>
      expect(state.db.workspaces.get(WS)?.archived).toBe(true)
    )
    await waitFor(() => expect(router.state.location.pathname).toBe("/"))
  })
})
