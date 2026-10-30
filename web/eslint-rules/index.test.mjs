import { RuleTester } from "eslint"
import { describe, it } from "vitest"

import plugin from "./index.mjs"

RuleTester.describe = describe
RuleTester.it = it
RuleTester.itOnly = it.only

const tester = new RuleTester({
  languageOptions: {
    ecmaVersion: 2022,
    sourceType: "module",
    parserOptions: { ecmaFeatures: { jsx: true } },
  },
})

tester.run("no-physical-direction", plugin.rules["no-physical-direction"], {
  valid: [
    {
      code: 'const a = "ms-2 me-2 ps-3 pe-3 start-0 end-0 text-start text-end rounded-s border-e"',
    },
    { code: 'const a = "left-aligned labels, right-click menu, ml-papers"' },
    { code: "const a = `flex ${x} gap-2`" },
    { code: 'const a = "rtl:-scale-x-100 md:ms-4"' },
  ],
  invalid: [
    ...[
      "ml-2",
      "mr-4",
      "pl-1",
      "pr-1",
      "left-0",
      "right-2",
      "text-left",
      "text-right",
      "rounded-l-md",
      "border-r",
      "md:ml-2",
      "-ml-1",
      "left-1/2",
      "right-[10px]",
      "mr-px",
      "ml-auto",
      "hover:right-0",
      "float-left",
    ].map((cls) => ({
      code: `const a = "flex ${cls} gap-2"`,
      errors: 1,
    })),
    { code: "const a = `p-2 ${x} ml-3`", errors: 1 },
  ],
})

tester.run("no-color-literal", plugin.rules["no-color-literal"], {
  valid: [
    { code: 'const a = "var(--primary)"' },
    { code: 'const a = "bg-primary text-foreground"' },
    { code: 'const a = "#main"' },
    {
      code: 'const a = "color-mix(in oklch, var(--foreground) 9%, transparent)"',
    },
  ],
  invalid: [
    { code: 'const a = "#fff"', errors: 1 },
    { code: 'const a = "#a78bfa"', errors: 1 },
    { code: 'const a = "#a78bfacc"', errors: 1 },
    { code: 'const a = "rgb(0 0 0)"', errors: 1 },
    { code: 'const a = "oklch(0.5 0.1 200)"', errors: 1 },
    { code: 'const a = "bg-[#0a0810]"', errors: 1 },
    { code: "const a = `hsl(${h} 50% 50%)`", errors: 1 },
  ],
})

tester.run("no-layout-transition", plugin.rules["no-layout-transition"], {
  valid: [
    { code: 'const a = "transition-[transform,opacity] duration-200"' },
    { code: 'const a = "transition-[stroke-dashoffset]"' },
    { code: 'const a = "transition-colors"' },
  ],
  invalid: [
    { code: 'const a = "transition-all"', errors: 1 },
    { code: 'const a = "transition-[width]"', errors: 1 },
    { code: 'const a = "hover:transition-[height,opacity]"', errors: 1 },
    { code: 'const a = "transition-[left]"', errors: 1 },
  ],
})
