import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, renderHook, waitFor } from "@testing-library/react"
import { HttpResponse, delay, http } from "msw"
import type { ReactNode } from "react"
import { describe, expect, it } from "vitest"

import { server } from "@/test/msw/server"

import { keys } from "../keys"
import type { ReviewQueue, ReviewStats } from "../pending-types"

import { useGradeCard } from "./review"

const WS = "ws_t"
const card = (id: string) => ({ id }) as unknown as ReviewQueue["items"][number]
const queue = (): ReviewQueue =>
  ({ items: [card("c1"), card("c2")], nextCursor: null }) as ReviewQueue
const stats = (): ReviewStats =>
  ({ dueNow: 2, reviewedToday: 5 }) as ReviewStats

function setup() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  qc.setQueryData(keys.ws(WS).reviewQueue(), queue())
  qc.setQueryData(keys.ws(WS).reviewStats(), stats())
  const wrapper = ({ children }: { children: ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  )
  return { qc, ...renderHook(() => useGradeCard(WS), { wrapper }) }
}

describe("optimistic review grade", () => {
  it("removes the card and bumps stats immediately, before the server answers", async () => {
    server.use(
      http.post("*/api/v1/workspaces/:ws/review/:id/grade", async () => {
        await delay(80)
        return HttpResponse.json({ id: "c1" })
      })
    )
    const { qc, result } = setup()
    act(() => result.current.mutate({ cardId: "c1", rating: 3 }))
    await waitFor(() =>
      expect(
        qc
          .getQueryData<ReviewQueue>(keys.ws(WS).reviewQueue())
          ?.items.map((c) => c.id)
      ).toEqual(["c2"])
    )
    expect(
      qc.getQueryData<ReviewStats>(keys.ws(WS).reviewStats())
    ).toMatchObject({
      dueNow: 1,
      reviewedToday: 6,
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
  })

  it("rolls the queue and stats back when the server rejects the grade", async () => {
    server.use(
      http.post("*/api/v1/workspaces/:ws/review/:id/grade", async () => {
        await delay(40)
        return HttpResponse.json(
          {
            error: {
              code: "internal",
              message: "boom",
              requestId: "r",
              retryable: false,
            },
          },
          { status: 500 }
        )
      })
    )
    const { qc, result } = setup()
    act(() => result.current.mutate({ cardId: "c1", rating: 1 }))
    await waitFor(() => expect(result.current.isError).toBe(true))
    expect(
      qc
        .getQueryData<ReviewQueue>(keys.ws(WS).reviewQueue())
        ?.items.map((c) => c.id)
    ).toEqual(["c1", "c2"])
    expect(
      qc.getQueryData<ReviewStats>(keys.ws(WS).reviewStats())
    ).toMatchObject({
      dueNow: 2,
      reviewedToday: 5,
    })
  })

  it("sends the same client id so a retried grade is applied once", async () => {
    const seen: string[] = []
    server.use(
      http.post(
        "*/api/v1/workspaces/:ws/review/:id/grade",
        async ({ request }) => {
          const body = (await request.json()) as {
            clientId: string
            rating: number
          }
          seen.push(body.clientId)
          return HttpResponse.json({ id: "c1" })
        }
      )
    )
    const { result } = setup()
    act(() =>
      result.current.mutate({ cardId: "c1", rating: 3, clientId: "fixed-id" })
    )
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(seen).toEqual(["fixed-id"])
  })
})
