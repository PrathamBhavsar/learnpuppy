import { describe, expect, it } from "vitest"

import { diffProfiles, providerLabel, withProvider } from "./model"

const base = {
  provider: "claude",
  model: "claude-sonnet-5-5",
  effort: "medium",
  fallback: true,
}

describe("diffProfiles", () => {
  it("is empty for equal maps", () => {
    expect(diffProfiles({ teacher: base }, { teacher: { ...base } })).toEqual(
      []
    )
  })

  it("lists each changed field with before and after", () => {
    const changes = diffProfiles(
      { teacher: base },
      {
        teacher: {
          ...base,
          model: "claude-opus-5-5",
          effort: null,
          fallback: false,
        },
      }
    )
    expect(changes).toEqual([
      {
        role: "teacher",
        field: "model",
        from: "claude-sonnet-5-5",
        to: "claude-opus-5-5",
      },
      { role: "teacher", field: "effort", from: "medium", to: "default" },
      { role: "teacher", field: "fallback", from: "on", to: "off" },
    ])
  })

  it("treats a role that appears or disappears as an override toggle", () => {
    expect(diffProfiles({}, { prober: base })).toEqual([
      {
        role: "prober",
        field: "override",
        from: "inherit global",
        to: "override",
      },
    ])
    expect(diffProfiles({ prober: base }, {})).toEqual([
      {
        role: "prober",
        field: "override",
        from: "override",
        to: "inherit global",
      },
    ])
  })
})

describe("withProvider", () => {
  it("keeps a model the new provider offers and otherwise picks its first", () => {
    expect(withProvider(base, "codex", ["gpt-5.5", "gpt-5.5-mini"]).model).toBe(
      "gpt-5.5"
    )
    expect(withProvider(base, "claude", ["claude-sonnet-5-5"]).model).toBe(
      "claude-sonnet-5-5"
    )
  })
})

describe("providerLabel", () => {
  it("names the known providers and passes unknown ids through", () => {
    expect(providerLabel("agy")).toBe("Antigravity")
    expect(providerLabel("other")).toBe("other")
  })
})
