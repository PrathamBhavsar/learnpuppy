// Color math shared by hex2oklch.ts (token generation) and contrast.ts (WCAG gate).
export type Oklch = { l: number; c: number; h: number; a?: number }

export function hexToOklch(hex: string): Oklch {
  const n = parseInt(hex.replace("#", ""), 16)
  const lin = (v: number) => {
    const s = v / 255
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }
  const r = lin((n >> 16) & 255)
  const g = lin((n >> 8) & 255)
  const b = lin(n & 255)
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  const C = Math.hypot(A, B)
  const H = C < 1e-4 ? 0 : ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360
  return { l: L, c: C, h: H }
}

/** OKLCH -> linear sRGB, clamped to the gamut. */
export function oklchToLinear({ l, c, h }: Oklch): [number, number, number] {
  const a = c * Math.cos((h * Math.PI) / 180)
  const b = c * Math.sin((h * Math.PI) / 180)
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3
  const clamp = (v: number) => Math.min(1, Math.max(0, v))
  return [
    clamp(4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_),
    clamp(-1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_),
    clamp(-0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_),
  ]
}

export function luminance(color: Oklch): number {
  const [r, g, b] = oklchToLinear(color)
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

/** WCAG 2 contrast ratio. A translucent foreground is composited over `bg`. */
export function contrast(fg: Oklch, bg: Oklch): number {
  const lf = luminance(fg.a === undefined ? fg : compositeOver(fg, bg))
  const lb = luminance(bg)
  const [hi, lo] = lf > lb ? [lf, lb] : [lb, lf]
  return (hi + 0.05) / (lo + 0.05)
}

/** Linear sRGB -> OKLCH (inverse of `oklchToLinear` inside the gamut). */
export function linearToOklch([r, g, b]: [number, number, number]): Oklch {
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  const C = Math.hypot(A, B)
  return {
    l: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    c: C,
    h: C < 1e-4 ? 0 : ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360,
  }
}

const encode = (v: number) =>
  v <= 0.0031308 ? v * 12.92 : 1.055 * v ** (1 / 2.4) - 0.055
const decode = (v: number) =>
  v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4

/** The opaque color you see when `top` (with its alpha) is painted over `base`. Browsers blend gamma-encoded sRGB. */
export function compositeOver(top: Oklch, base: Oklch): Oklch {
  const alpha = top.a ?? 1
  const t = oklchToLinear(top).map(encode)
  const b = oklchToLinear(base).map(encode)
  return linearToOklch(
    [0, 1, 2].map((i) => decode(t[i]! * alpha + b[i]! * (1 - alpha))) as [
      number,
      number,
      number,
    ]
  )
}

/** Move OKLCH lightness (keeping chroma and hue) until `color` reaches `min` contrast on `bg`. */
export function fitContrast(color: Oklch, bg: Oklch, min: number): Oklch {
  const dir = luminance(bg) > 0.4 ? -1 : 1
  let { l } = color
  while (l > 0.05 && l < 0.99 && contrast({ ...color, l }, bg) < min) {
    l += dir * 0.005
  }
  return { ...color, l }
}

const fmt = (n: number, d = 3) => String(+n.toFixed(d))

export function toCss({ l, c, h, a }: Oklch): string {
  const alpha = a !== undefined ? ` / ${Math.round(a * 100)}%` : ""
  return `oklch(${fmt(l)} ${fmt(c)} ${fmt(h, 2)}${alpha})`
}
