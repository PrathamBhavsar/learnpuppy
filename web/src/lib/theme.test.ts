import { describe, expect, it } from "vitest"

import themeInit from "../../public/theme-init.js?raw"

import { mockMatchMedia } from "@/test/media"

import { readDirection, readTheme, resolveTheme } from "./theme"

const DARK = "(prefers-color-scheme: dark)"

describe("theme helpers", () => {
  it("resolves system from prefers-color-scheme", () => {
    mockMatchMedia({ [DARK]: true })
    expect(resolveTheme("system")).toBe("dark")
    mockMatchMedia({ [DARK]: false })
    expect(resolveTheme("system")).toBe("light")
    expect(resolveTheme("dark")).toBe("dark")
  })

  it("falls back to system for missing or corrupt storage", () => {
    expect(readTheme()).toBe("system")
    localStorage.setItem("lp-theme", "neon")
    expect(readTheme()).toBe("system")
    localStorage.setItem("lp-theme", "light")
    expect(readTheme()).toBe("light")
  })

  it("lets ?dir= override the stored direction", () => {
    expect(readDirection("")).toBe("ltr")
    localStorage.setItem("lp-dir", "rtl")
    expect(readDirection("")).toBe("rtl")
    expect(readDirection("?dir=ltr")).toBe("ltr")
    expect(readDirection("?dir=rtl")).toBe("rtl")
    expect(readDirection("?dir=sideways")).toBe("rtl")
  })
})

describe("public/theme-init.js (pre-paint script)", () => {
  const run = () => new Function(themeInit)()

  it("applies the stored theme and direction before React mounts", () => {
    localStorage.setItem("lp-theme", "light")
    localStorage.setItem("lp-dir", "rtl")
    run()
    const root = document.documentElement
    expect(root).toHaveClass("light")
    expect(root.dir).toBe("rtl")
    expect(root.style.colorScheme).toBe("light")
  })

  it("follows the OS when the theme is system", () => {
    mockMatchMedia({ [DARK]: true })
    run()
    expect(document.documentElement).toHaveClass("dark")
  })

  it("restores the reduce-motion override", () => {
    localStorage.setItem("lp-reduce-motion", "1")
    run()
    expect(document.documentElement.dataset.reduceMotion).toBe("true")
  })
})
