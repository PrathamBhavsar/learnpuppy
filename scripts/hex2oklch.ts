#!/usr/bin/env bun
import assert from "node:assert/strict";

/** Convert sRGB hex to OKLCH; surface lifts add to perceptual lightness. */
export function hexToOklch(hex: string, lift = 0, alpha = 1): string {
  if (!/^#(?:[\da-f]{3}|[\da-f]{6})$/i.test(hex)) {
    throw new Error(`Invalid RGB hex color: ${hex}`);
  }
  if (!Number.isFinite(lift) || !Number.isFinite(alpha) || alpha < 0 || alpha > 1) {
    throw new Error("Lightness lift must be finite and alpha must be between 0 and 1");
  }
  const rgb = hex.length === 4
    ? [...hex.slice(1)].map((digit) => digit + digit).join("")
    : hex.slice(1);
  const [r, g, b] = [0, 2, 4].map((offset) => {
    const value = parseInt(rgb.slice(offset, offset + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  });
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const lightness = Math.max(0, Math.min(1, 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s + lift));
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const labB = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  const chroma = Math.hypot(a, labB);
  const hue = chroma < 1e-7 ? 0 : (Math.atan2(labB, a) * 180 / Math.PI + 360) % 360;
  const number = (value: number, digits = 6) => Number(value.toFixed(digits));
  return `oklch(${number(lightness)} ${number(chroma)} ${number(hue, 3)}${alpha === 1 ? "" : ` / ${number(alpha * 100)}%`})`;
}

function hexToOklchParts(hex: string): { l: number; c: number; h: number } {
  const [l, c, h] = hexToOklch(hex).match(/[\d.]+/g)!.map(Number);
  return { l, c, h };
}

/** Lower OKLCH lightness (keeping chroma and hue) until the color reaches `min` WCAG contrast on white. */
export function darkenForWhite(hex: string, min = 3.2): string {
  const base = hexToOklchParts(hex);
  const toLin = (L: number, C: number, H: number) => {
    const a = C * Math.cos((H * Math.PI) / 180);
    const b = C * Math.sin((H * Math.PI) / 180);
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    const g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    const bl = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
    return [r, g, bl].map((v) => Math.min(1, Math.max(0, v)));
  };
  const contrast = (L: number) => {
    const [r, g, b] = toLin(L, base.c, base.h);
    return 1.05 / (0.2126 * r + 0.7152 * g + 0.0722 * b + 0.05);
  };
  let L = base.l;
  while (L > 0.2 && contrast(L) < min) L -= 0.005;
  const n = (v: number, d = 6) => Number(v.toFixed(d));
  return `oklch(${n(L)} ${n(base.c)} ${n(base.h, 3)})`;
}

/** Torlink hex values from docs/DESIGN_SYSTEM_PLAN.md; preset neutral light surfaces. */
export function themeTokens(): string {
  const color = hexToOklch;
  const background = color("#0a0810");
  const foreground = color("#e9e4f5");
  const primary = color("#a78bfa");
  const accent = color("#d8b4fe");
  const surface = color("#0a0810", 0.04);
  const muted = color("#0a0810", 0.08);
  const shared = {
    destructive: color("#ee7d92"),
    "destructive-foreground": background,
    success: color("#86d6a2"),
    "success-foreground": background,
    warning: color("#f0c560"),
    "warning-foreground": background,
    info: color("#7db8f0"),
    "info-foreground": background,
    "chart-1": primary,
    "chart-2": accent,
    "chart-3": color("#5fd0c5"),
    "chart-4": color("#f6a55c"),
    "chart-5": color("#86d6a2"),
    "mastery-novice": color("#ee7d92"),
    "mastery-learning": color("#f0c560"),
    "mastery-mastered": color("#86d6a2"),
    radius: "0.625rem",
    "font-sans": '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif',
    "font-mono": '"Geist Mono", "SF Mono", "JetBrains Mono", ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace',
    "font-heading": "var(--font-mono)",
  };
  const dark = {
    background,
    foreground,
    card: surface,
    "card-foreground": foreground,
    popover: surface,
    "popover-foreground": foreground,
    primary,
    "primary-foreground": background,
    secondary: muted,
    "secondary-foreground": foreground,
    muted,
    "muted-foreground": color("#b9a7e6"),
    accent,
    "accent-foreground": background,
    border: color("#6b6577", 0, 0.3),
    input: color("#6b6577", 0, 0.4),
    ring: primary,
    sidebar: surface,
    "sidebar-foreground": foreground,
    "sidebar-primary": primary,
    "sidebar-primary-foreground": background,
    "sidebar-accent": muted,
    "sidebar-accent-foreground": foreground,
    "sidebar-border": color("#6b6577", 0, 0.3),
    "sidebar-ring": primary,
    ...shared,
  };
  const lightPrimary = color("#7c5cd6");
  const light = {
    ...dark,
    background: color("#ffffff"),
    foreground: color("#0a0a0a"),
    card: color("#ffffff"),
    "card-foreground": color("#0a0a0a"),
    popover: color("#ffffff"),
    "popover-foreground": color("#0a0a0a"),
    primary: lightPrimary,
    "primary-foreground": "oklch(0.985 0 0)",
    secondary: color("#f5f5f5"),
    "secondary-foreground": color("#171717"),
    muted: color("#f5f5f5"),
    "muted-foreground": color("#737373"),
    accent: "oklch(0.205 0 0)",
    "accent-foreground": "oklch(0.985 0 0)",
    destructive: "oklch(0.577 0.245 27.325)",
    "destructive-foreground": color("#fafafa"),
    border: color("#e5e5e5"),
    input: color("#e5e5e5"),
    ring: lightPrimary,
    sidebar: color("#fafafa"),
    "sidebar-foreground": color("#0a0a0a"),
    "sidebar-primary": "oklch(0.205 0 0)",
    "sidebar-primary-foreground": "oklch(0.985 0 0)",
    "sidebar-accent": "oklch(0.97 0 0)",
    "sidebar-accent-foreground": "oklch(0.205 0 0)",
    "sidebar-border": color("#e5e5e5"),
    "sidebar-ring": "oklch(0.708 0 0)",
    // Darker light-mode variants: >=3:1 on white.
    success: darkenForWhite("#86d6a2"),
    warning: darkenForWhite("#f0c560"),
    info: darkenForWhite("#7db8f0"),
    "chart-1": darkenForWhite("#a78bfa"),
    "chart-2": darkenForWhite("#d8b4fe"),
    "chart-3": darkenForWhite("#5fd0c5"),
    "chart-4": darkenForWhite("#f6a55c"),
    "chart-5": darkenForWhite("#86d6a2"),
    "mastery-novice": darkenForWhite("#ee7d92"),
    "mastery-learning": darkenForWhite("#f0c560"),
    "mastery-mastered": darkenForWhite("#86d6a2"),
  };
  const block = (selector: string, scheme: string, tokens: Record<string, string>) =>
    `${selector} {\n  color-scheme: ${scheme};\n${Object.entries(tokens).map(([name, value]) => `  --${name}: ${value};`).join("\n")}\n}`;
  return `${block(":root, .dark", "dark", dark)}\n\n${block(".light", "light", light)}`;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--tokens") {
    console.log(themeTokens());
  } else if (args.length === 1 && args[0] === "--test") {
    assert.equal(hexToOklch("#000000"), "oklch(0 0 0)");
    assert.equal(hexToOklch("#fff"), "oklch(1 0 0)");
    assert.equal(hexToOklch("#ff0000"), "oklch(0.627955 0.257683 29.234)");
    assert.equal(hexToOklch("#000", 0.04, 0.3), "oklch(0.04 0 0 / 30%)");
    assert.equal(hexToOklch("#fff", 0.08), "oklch(1 0 0)");
    assert.equal(hexToOklch("#000", -0.08, 0), "oklch(0 0 0 / 0%)");
    assert.throws(() => hexToOklch("#gggggg"));
    assert.throws(() => hexToOklch("#12345"));
    assert.throws(() => hexToOklch("#fff", NaN));
    assert.throws(() => hexToOklch("#fff", 0, -1));
    assert.throws(() => hexToOklch("#fff", 0, 2));
    assert.throws(() => hexToOklch("#fff", 0, NaN));
    console.log("hex2oklch self-checks passed");
  } else if (args.length > 0 && args.every((arg) => arg.startsWith("#"))) {
    for (const hex of args) console.log(hexToOklch(hex));
  } else {
    console.error("Usage: bun scripts/hex2oklch.ts '#a78bfa' [...] | --tokens | --test");
    process.exitCode = 1;
  }
}
