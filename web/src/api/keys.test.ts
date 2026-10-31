import { describe, expect, it } from "vitest"

import { keys } from "./keys"

describe("query keys", () => {
  it("scopes every workspace key under ['ws', id]", () => {
    const k = keys.ws("a")
    const all = [
      k.detail(),
      k.graph(),
      k.graphSnapshot(),
      k.graphNode("n"),
      k.graphSearch("q"),
      k.graphChains(["x"]),
      k.chats(),
      k.chat("c"),
      k.jobs(),
      k.review(),
      k.reviewQueue(5),
      k.reviewStats(),
      k.reviewPreview("p"),
      k.reviewCards({ state: "new" }),
      k.digest(),
      k.digestAll(),
      k.decision("d"),
      k.ingestSources(),
      k.ingestRuns(),
      k.ingest(),
    ]
    for (const key of all) expect(key.slice(0, 2)).toEqual(["ws", "a"])
  })

  it("keeps different workspaces apart", () => {
    expect(keys.ws("a").graph()).not.toEqual(keys.ws("b").graph())
  })

  it("nests prefixes so one invalidation covers a whole branch", () => {
    const k = keys.ws("a")
    expect(k.graphNode("n").slice(0, 2 + 1)).toEqual(k.graph().slice(0, 3))
    expect(k.reviewQueue().slice(0, 3)).toEqual(k.review())
    expect(k.decision("d").slice(0, 3)).toEqual(k.digestAll())
    expect(keys.workspaces(true).slice(0, 1)).toEqual(keys.workspacesAll())
  })

  it("normalises optional filters so equivalent calls share a cache entry", () => {
    const k = keys.ws("a")
    expect(k.reviewCards()).toEqual(k.reviewCards({}))
    expect(k.digest()).toEqual(k.digest({}))
    expect(k.reviewQueue()).toEqual(k.reviewQueue(undefined))
  })

  it("sorts chain selections so order does not fragment the cache", () => {
    const k = keys.ws("a")
    expect(k.graphChains(["b", "a"])).toEqual(k.graphChains(["a", "b"]))
  })
})
