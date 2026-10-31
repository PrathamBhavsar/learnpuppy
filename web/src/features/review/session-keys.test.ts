import { describe, expect, it } from "vitest"

import { resolveSessionKey } from "./session-keys"

const key = (k: string, extra: object = {}) => ({ key: k, ...extra })

describe("session keyboard contract", () => {
  it("chooses with 1-4 and a-d while answering", () => {
    expect(resolveSessionKey("answering", 4, key("1"))).toEqual({
      type: "choose",
      index: 0,
    })
    expect(resolveSessionKey("answering", 4, key("4"))).toEqual({
      type: "choose",
      index: 3,
    })
    expect(resolveSessionKey("answering", 4, key("c"))).toEqual({
      type: "choose",
      index: 2,
    })
    expect(resolveSessionKey("answering", 4, key("D"))).toEqual({
      type: "choose",
      index: 3,
    })
  })

  it("ignores choices beyond the option count and other keys", () => {
    expect(resolveSessionKey("answering", 2, key("3"))).toBeNull()
    expect(resolveSessionKey("answering", 4, key("x"))).toBeNull()
    expect(resolveSessionKey("answering", 4, key(" "))).toBeNull()
  })

  it("rates with 1-4 and accepts the suggestion with Space once answered", () => {
    expect(resolveSessionKey("answered", 4, key("1"))).toEqual({
      type: "rate",
      rating: 1,
    })
    expect(resolveSessionKey("answered", 4, key("4"))).toEqual({
      type: "rate",
      rating: 4,
    })
    expect(resolveSessionKey("answered", 4, key(" "))).toEqual({
      type: "accept",
    })
  })

  it("does not treat letters as ratings", () => {
    expect(resolveSessionKey("answered", 4, key("a"))).toBeNull()
    expect(resolveSessionKey("answered", 4, key("d"))).toBeNull()
  })

  it("exits on Escape mid-card only", () => {
    expect(resolveSessionKey("answering", 4, key("Escape"))).toEqual({
      type: "exit",
    })
    expect(resolveSessionKey("answered", 4, key("Escape"))).toEqual({
      type: "exit",
    })
    expect(resolveSessionKey("complete", 4, key("Escape"))).toBeNull()
    expect(resolveSessionKey("idle", 4, key("Escape"))).toBeNull()
  })

  it("never fires with modifiers (browser and app shortcuts keep working)", () => {
    expect(
      resolveSessionKey("answering", 4, key("1", { metaKey: true }))
    ).toBeNull()
    expect(
      resolveSessionKey("answered", 4, key("2", { ctrlKey: true }))
    ).toBeNull()
    expect(
      resolveSessionKey("answering", 4, key("a", { altKey: true }))
    ).toBeNull()
  })
})
