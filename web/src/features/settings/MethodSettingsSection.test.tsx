import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { learningFixture } from "@/test/msw/fixtures/learning"

import { MethodSettingsSection } from "./MethodSettingsSection"

const WS = "ws_rustasync"

function mount() {
  state.db.learning.set(WS, learningFixture())
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <MethodSettingsSection ws={WS} />
    </QueryClientProvider>
  )
}

describe("MethodSettingsSection", () => {
  it("lists every setting with its range and default", async () => {
    mount()
    const input = await screen.findByTestId("method-attempts_before_parking")
    expect(input).toHaveValue(3)
    expect(screen.getByText(/Allowed 1 to 10; default 3\./)).toBeInTheDocument()
  })

  it("saves a changed value on blur and offers Reset once it differs from the default", async () => {
    const user = userEvent.setup()
    mount()
    const input = await screen.findByTestId("method-attempts_before_parking")
    await user.clear(input)
    await user.type(input, "5")
    await user.tab()
    await waitFor(() =>
      expect(
        state.db.learning
          .get(WS)
          ?.settings.find((s) => s.name === "attempts_before_parking")?.value
      ).toBe(5)
    )
    const reset = await screen.findByRole("button", { name: "Reset" })
    await user.click(reset)
    await waitFor(() => expect(input).toHaveValue(3))
    expect(
      screen.queryByRole("button", { name: "Reset" })
    ).not.toBeInTheDocument()
  })

  it("puts the old value back when the server refuses the new one", async () => {
    const user = userEvent.setup()
    mount()
    const input = await screen.findByTestId("method-attempts_before_parking")
    await user.clear(input)
    await user.type(input, "99")
    await user.tab()
    await waitFor(() => expect(input).toHaveValue(3))
    expect(
      state.db.learning
        .get(WS)
        ?.settings.find((s) => s.name === "attempts_before_parking")?.value
    ).toBe(3)
  })
})
