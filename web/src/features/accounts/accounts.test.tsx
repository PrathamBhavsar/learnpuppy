import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it, vi } from "vitest"

import { state } from "@/test/msw/db"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

import {
  cooldownFraction,
  formatCountdown,
  formatTokens,
  groupAccounts,
  reauthCommand,
} from "./model"

beforeEach(resetUi)

const API = "*/api/v1"

async function open() {
  const view = renderApp("/accounts")
  await screen.findByTestId("screen-accounts")
  return { ...view, user: userEvent.setup() }
}

describe("model", () => {
  it("formats tokens and countdowns", () => {
    expect(formatTokens(950)).toBe("950")
    expect(formatTokens(12_400)).toBe("12.4k")
    expect(formatTokens(412_000)).toBe("412k")
    expect(formatTokens(2_500_000)).toBe("2.5M")
    expect(formatCountdown(42_000)).toBe("42s")
    expect(formatCountdown(125_000)).toBe("2m 05s")
    expect(formatCountdown(-5)).toBe("0s")
  })

  it("clamps the cooldown fraction", () => {
    expect(cooldownFraction(30_000, 60_000)).toBe(0.5)
    expect(cooldownFraction(90_000, 60_000)).toBe(1)
    expect(cooldownFraction(0, 0)).toBe(0)
  })

  it("groups by provider in a fixed order", () => {
    const mk = (provider: string, id: string) =>
      ({
        id,
        provider,
        status: "ready",
        requests: 0,
        tokensIn: 0,
        tokensOut: 0,
      }) as const
    const groups = groupAccounts([
      mk("agy", "a"),
      mk("claude", "b"),
      mk("claude", "c"),
      mk("zed", "d"),
    ])
    expect(groups.map((g) => [g.provider, g.accounts.length])).toEqual([
      ["claude", 2],
      ["agy", 1],
      ["zed", 1],
    ])
  })

  it("prefers the server's re-auth hint and falls back for expired logins", () => {
    const base = {
      id: "x",
      provider: "claude",
      requests: 0,
      tokensIn: 0,
      tokensOut: 0,
    }
    expect(
      reauthCommand({
        ...base,
        status: "auth_expired",
        reauthHint: "CLAUDE_CONFIG_DIR=/a claude login",
      })
    ).toBe("CLAUDE_CONFIG_DIR=/a claude login")
    expect(reauthCommand({ ...base, status: "auth_expired" })).toBe(
      "claude login"
    )
    expect(reauthCommand({ ...base, status: "ready" })).toBeNull()
  })
})

describe("Accounts: five states", () => {
  it("loading: skeleton cards first", async () => {
    renderApp("/accounts")
    expect(await screen.findByTestId("accounts-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("accounts-list")).toBeInTheDocument()
  })

  it("populated: provider groups, status labels and usage", async () => {
    await open()
    const claude = await screen.findByTestId("accounts-group-claude")
    expect(within(claude).getAllByRole("listitem")).toHaveLength(2)
    expect(screen.getByTestId("accounts-group-codex")).toBeInTheDocument()
    expect(screen.getByTestId("accounts-group-agy")).toBeInTheDocument()
    expect(screen.getByTestId("account-status-claude:acc-1")).toHaveTextContent(
      "Ready"
    )
    expect(screen.getByTestId("account-status-claude:acc-2")).toHaveTextContent(
      "Cooling down"
    )
    expect(screen.getByTestId("account-tier-claude:acc-1")).toHaveTextContent(
      "max 5x"
    )
  })

  it("empty: explains how to sign in and offers a rescan", async () => {
    state.db.accounts = []
    await open()
    const empty = await screen.findByTestId("empty-state")
    expect(empty).toHaveTextContent("No accounts found")
    expect(within(empty).getByTestId("accounts-rescan")).toBeInTheDocument()
  })

  it("error: shows the server message and retries", async () => {
    let calls = 0
    server.use(
      http.get(`${API}/accounts`, () => {
        calls += 1
        return calls === 1
          ? apiError("internal", "Pool unavailable")
          : HttpResponse.json({ items: state.db.accounts })
      })
    )
    const { user } = await open()
    expect(await screen.findByTestId("error-state")).toHaveTextContent(
      "Pool unavailable"
    )
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByTestId("accounts-list")).toBeInTheDocument()
  })

  it("degraded: no ready account shows the fix-it alert", async () => {
    for (const a of state.db.accounts) a.status = "disabled"
    await open()
    expect(await screen.findByTestId("accounts-none-ready")).toHaveTextContent(
      "No account is ready"
    )
  })
})

describe("Accounts: cards", () => {
  it("cooldown ring counts down the remaining time", async () => {
    await open()
    const ring = await screen.findByTestId("account-ring-claude:acc-2")
    expect(ring).toHaveAttribute("role", "timer")
    const first = screen.getByTestId(
      "account-ring-claude:acc-2-text"
    ).textContent
    expect(first).toMatch(/^\d+s$/)
    await waitFor(
      () =>
        expect(
          screen.getByTestId("account-ring-claude:acc-2-text").textContent
        ).not.toBe(first),
      { timeout: 3000 }
    )
  })

  it("an expired login shows the exact re-auth command and copies it", async () => {
    const { user } = await open()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    })
    const hint = await screen.findByTestId("account-reauth-agy:acc-1")
    expect(hint).toHaveTextContent("agy auth login")
    await user.click(screen.getByTestId("account-reauth-copy-agy:acc-1"))
    expect(writeText).toHaveBeenCalledWith("agy auth login")
  })

  it("the switch disables and re-enables an account", async () => {
    const { user } = await open()
    const toggle = await screen.findByTestId("account-toggle-codex:acc-1")
    expect(toggle).toBeChecked()
    await user.click(toggle)
    await waitFor(() =>
      expect(
        screen.getByTestId("account-status-codex:acc-1")
      ).toHaveTextContent("Disabled")
    )
    expect(state.db.accounts.find((a) => a.id === "codex:acc-1")?.status).toBe(
      "disabled"
    )
    await user.click(screen.getByTestId("account-toggle-codex:acc-1"))
    await waitFor(() =>
      expect(
        screen.getByTestId("account-status-codex:acc-1")
      ).toHaveTextContent("Ready")
    )
  })

  it("a new cooldown pulses the card border once", async () => {
    const { user } = await open()
    await screen.findByTestId("account-claude:acc-1")
    expect(screen.getByTestId("account-claude:acc-1")).toHaveAttribute(
      "data-pulse",
      "false"
    )
    const account = state.db.accounts.find((a) => a.id === "claude:acc-1")
    if (!account) throw new Error("fixture missing")
    account.status = "cooldown"
    account.cooldownUntil = Date.now() + 30_000
    await act(async () => {
      await user.click(screen.getByTestId("accounts-rescan"))
    })
    await waitFor(() =>
      expect(screen.getByTestId("account-claude:acc-1")).toHaveAttribute(
        "data-pulse",
        "true"
      )
    )
  })
})

describe("Accounts: add profile and rescan", () => {
  it("rejects a relative path before calling the server", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("accounts-add"))
    await user.type(
      await screen.findByTestId("add-profile-path"),
      "relative/dir"
    )
    await user.click(screen.getByTestId("add-profile-submit"))
    expect(await screen.findByTestId("add-profile-error")).toHaveTextContent(
      "absolute path"
    )
  })

  it("shows the server's field error for a directory that is not a profile", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("accounts-add"))
    await user.type(await screen.findByTestId("add-profile-path"), "/tmp/nope")
    await user.click(screen.getByTestId("add-profile-submit"))
    expect(await screen.findByTestId("add-profile-error")).toHaveTextContent(
      "No .claude.json"
    )
  })

  it("adds a profile and lists it", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("accounts-add"))
    await user.type(
      await screen.findByTestId("add-profile-path"),
      "/Users/dev/.claude-extra"
    )
    await user.click(screen.getByTestId("add-profile-submit"))
    await waitFor(() =>
      expect(screen.queryByTestId("add-profile-dialog")).not.toBeInTheDocument()
    )
    expect(
      await screen.findByTestId("account-claude:/Users/dev/.claude-extra")
    ).toBeInTheDocument()
  })

  it("rescan picks up a profile that appeared on disk", async () => {
    const { user } = await open()
    await screen.findByTestId("accounts-list")
    state.db.accounts.push({
      id: "codex:acc-2",
      provider: "codex",
      email: "new@example.com",
      tier: "plus",
      status: "ready",
      requests: 0,
      tokensIn: 0,
      tokensOut: 0,
    })
    await user.click(screen.getByTestId("accounts-rescan"))
    expect(await screen.findByTestId("account-codex:acc-2")).toBeInTheDocument()
  })
})

describe("Accounts: role slots", () => {
  it("edits a model, previews the diff, then saves", async () => {
    const { user } = await open()
    const model = await screen.findByTestId("global-model-teacher")
    await user.click(model)
    await user.click(
      await screen.findByRole("option", { name: "claude-sonnet-5-5" })
    )
    expect(screen.getByTestId("global-dirty")).toHaveTextContent(
      "1 unsaved change"
    )

    await user.click(screen.getByTestId("global-review"))
    const diff = await screen.findByTestId("global-diff-list")
    expect(diff).toHaveTextContent("claude-opus-5-5")
    expect(diff).toHaveTextContent("claude-sonnet-5-5")
    await user.click(screen.getByTestId("global-confirm"))

    await waitFor(() =>
      expect(state.db.settings.profiles["teacher"]?.model).toBe(
        "claude-sonnet-5-5"
      )
    )
    await waitFor(() =>
      expect(screen.queryByTestId("global-dirty")).not.toBeInTheDocument()
    )
  })

  it("changing provider moves the model to one the provider offers", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("global-provider-prober"))
    await user.click(await screen.findByRole("option", { name: "Codex" }))
    expect(screen.getByTestId("global-model-prober")).toHaveTextContent(
      "gpt-5.5"
    )
  })

  it("attaches the server's validation error to the offending row", async () => {
    server.use(
      http.put(`${API}/settings`, () =>
        apiError("validation", "Invalid settings", {
          details: {
            fields: [
              {
                path: "profiles.teacher.model",
                message: "not offered by claude",
              },
            ],
          },
        })
      )
    )
    const { user } = await open()
    await user.click(await screen.findByTestId("global-fallback-teacher"))
    await user.click(screen.getByTestId("global-review"))
    await user.click(await screen.findByTestId("global-confirm"))
    expect(
      await screen.findByTestId("global-model-error-teacher")
    ).toHaveTextContent("not offered by claude")
  })

  it("test ping reports latency, and failure text for an unavailable provider", async () => {
    const { user } = await open()
    await user.click(await screen.findByTestId("global-test-teacher"))
    expect(await screen.findByTestId("global-ping-teacher")).toHaveTextContent(
      "OK · 412 ms"
    )

    state.db.accounts = state.db.accounts.filter((a) => a.provider !== "codex")
    await user.click(screen.getByTestId("global-provider-prober"))
    await user.click(await screen.findByRole("option", { name: "Codex" }))
    await user.click(screen.getByTestId("global-test-prober"))
    expect(await screen.findByTestId("global-ping-prober")).toHaveTextContent(
      "No codex account is ready"
    )
  })

  it("says so when the server has no test endpoint", async () => {
    server.use(
      http.post(`${API}/settings/test`, () =>
        apiError("not_found", "no such route")
      )
    )
    const { user } = await open()
    await user.click(await screen.findByTestId("global-test-teacher"))
    expect(await screen.findByTestId("global-ping-teacher")).toHaveTextContent(
      "Not supported"
    )
  })
})
