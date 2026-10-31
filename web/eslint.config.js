import { readdirSync } from "node:fs"

import js from "@eslint/js"
import reactHooks from "eslint-plugin-react-hooks"
import reactRefresh from "eslint-plugin-react-refresh"
import jsxA11y from "eslint-plugin-jsx-a11y"
import { createTypeScriptImportResolver } from "eslint-import-resolver-typescript"
import { importX } from "eslint-plugin-import-x"
import globals from "globals"
import tseslint from "typescript-eslint"
import { defineConfig, globalIgnores } from "eslint/config"

import lp from "./eslint-rules/index.mjs"

// Feature isolation: features/* may not import each other (docs/web/04).
const features = readdirSync(new URL("./src/features", import.meta.url), {
  withFileTypes: true,
})
  .filter((entry) => entry.isDirectory())
  .map((entry) => entry.name)
const featureZones = features.map((name) => ({
  target: `./src/features/${name}`,
  from: "./src/features",
  except: [`./${name}`],
  message:
    "Features must not import each other; share through components/, lib/, stores/ or the palette registry.",
}))

export default defineConfig([
  globalIgnores([
    "dist",
    "mock",
    "playwright-report",
    "test-results",
    "src/routeTree.gen.ts",
  ]),
  {
    files: ["**/*.{ts,tsx}"],
    extends: [
      js.configs.recommended,
      tseslint.configs.recommended,
      reactHooks.configs.flat.recommended,
      reactRefresh.configs.vite,
      jsxA11y.flatConfigs.recommended,
    ],
    languageOptions: { globals: globals.browser },
    settings: {
      "import-x/resolver-next": [
        createTypeScriptImportResolver({ project: "./tsconfig.app.json" }),
      ],
    },
    plugins: { lp, "import-x": importX },
    rules: {
      "react-refresh/only-export-components": "off",
      "import-x/no-cycle": "error",
      "import-x/no-restricted-paths": ["error", { zones: featureZones }],
      "no-restricted-globals": [
        "error",
        { name: "fetch", message: "Network access goes through src/api." },
        {
          name: "EventSource",
          message: "Server-sent events go through src/api/events.ts.",
        },
        {
          name: "XMLHttpRequest",
          message: "Network access goes through src/api.",
        },
      ],
      "no-restricted-properties": [
        "error",
        {
          object: "window",
          property: "fetch",
          message: "Network access goes through src/api.",
        },
        {
          object: "globalThis",
          property: "fetch",
          message: "Network access goes through src/api.",
        },
        {
          object: "navigator",
          property: "sendBeacon",
          message: "Network access goes through src/api.",
        },
      ],
      // Preflight strips list styling, which makes Safari drop list semantics unless the role is explicit.
      "jsx-a11y/no-redundant-roles": ["error", { ul: ["list"], ol: ["list"] }],
      "lp/no-physical-direction": "error",
      "lp/no-color-literal": "error",
      "lp/no-layout-transition": "error",
    },
  },
  {
    // shadcn output is generated: its structure is the preset's, not ours to lint.
    files: ["src/components/ui/**"],
    rules: {
      "lp/no-physical-direction": "off",
      "lp/no-layout-transition": "off",
      "jsx-a11y/heading-has-content": "off",
      "jsx-a11y/anchor-has-content": "off",
      "jsx-a11y/click-events-have-key-events": "off",
      "jsx-a11y/no-noninteractive-element-interactions": "off",
    },
  },
  {
    files: ["src/api/**"],
    rules: {
      "no-restricted-globals": "off",
      "no-restricted-properties": "off",
    },
  },
  {
    files: [
      "**/*.test.{ts,tsx}",
      "src/test/**",
      "e2e/**",
      "scripts/**",
      "vite.config.ts",
      "vitest.config.ts",
      "playwright.config.ts",
    ],
    rules: {
      "lp/no-color-literal": "off",
      "no-restricted-globals": "off",
      "no-restricted-properties": "off",
    },
  },
  {
    files: ["scripts/**", "e2e/**", "*.config.ts", "eslint-rules/**"],
    languageOptions: { globals: globals.node },
  },
])
