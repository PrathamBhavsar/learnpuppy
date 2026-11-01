import { describe, expect, it } from "vitest"

import { validateGraphSearch } from "./use-graph-url"

describe("validateGraphSearch", () => {
  it("keeps non-empty strings for the supported keys", () => {
    expect(
      validateGraphSearch({
        node: "n_1",
        q: "wake",
        src: "claude,git",
        cluster: "cl_2",
        dir: "rtl",
      })
    ).toEqual({
      node: "n_1",
      q: "wake",
      src: "claude,git",
      cluster: "cl_2",
      dir: "rtl",
    })
  })

  it("drops empty, numeric and unknown values", () => {
    const parsed = validateGraphSearch({
      node: "",
      q: 5,
      src: null,
      cluster: undefined,
      other: "x",
    })
    expect(parsed).toEqual({
      node: undefined,
      q: undefined,
      src: undefined,
      cluster: undefined,
      dir: undefined,
    })
    expect(parsed).not.toHaveProperty("other")
  })
})
