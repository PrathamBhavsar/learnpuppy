import type { HighlighterCore } from "shiki/core"

/** Per-language loaders: each becomes its own chunk, fetched the first time that language appears. */
const LANGS: Record<string, () => Promise<{ default: unknown }>> = {
  rust: () => import("shiki/langs/rust.mjs"),
  typescript: () => import("shiki/langs/typescript.mjs"),
  tsx: () => import("shiki/langs/tsx.mjs"),
  javascript: () => import("shiki/langs/javascript.mjs"),
  json: () => import("shiki/langs/json.mjs"),
  bash: () => import("shiki/langs/bash.mjs"),
  python: () => import("shiki/langs/python.mjs"),
  toml: () => import("shiki/langs/toml.mjs"),
  yaml: () => import("shiki/langs/yaml.mjs"),
  html: () => import("shiki/langs/html.mjs"),
  css: () => import("shiki/langs/css.mjs"),
  sql: () => import("shiki/langs/sql.mjs"),
  go: () => import("shiki/langs/go.mjs"),
  c: () => import("shiki/langs/c.mjs"),
  cpp: () => import("shiki/langs/cpp.mjs"),
  diff: () => import("shiki/langs/diff.mjs"),
}

const ALIASES: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  js: "javascript",
  jsx: "tsx",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  py: "python",
  yml: "yaml",
  "c++": "cpp",
}

export function resolveLanguage(lang: string | undefined): string | null {
  if (!lang) return null
  const key = lang.toLowerCase()
  const name = ALIASES[key] ?? key
  return name in LANGS ? name : null
}

let highlighter: Promise<HighlighterCore> | null = null
const loaded = new Set<string>()

function core(): Promise<HighlighterCore> {
  highlighter ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }] =
      await Promise.all([
        import("shiki/core"),
        import("shiki/engine/javascript"),
      ])
    const [light, dark] = await Promise.all([
      import("shiki/themes/github-light.mjs"),
      import("shiki/themes/github-dark.mjs"),
    ])
    return createHighlighterCore({
      themes: [light.default, dark.default],
      langs: [],
      engine: createJavaScriptRegexEngine(),
    })
  })()
  return highlighter
}

/**
 * Highlighted HTML for `code`, or null for languages we do not bundle (rendered as plain text).
 * Shiki escapes the source, so the markup is safe to inject. Colors are CSS variables, one set per theme.
 */
export async function highlight(
  code: string,
  lang: string | undefined
): Promise<string | null> {
  const name = resolveLanguage(lang)
  const loadLang = name ? LANGS[name] : undefined
  if (!name || !loadLang) return null
  const instance = await core()
  if (!loaded.has(name)) {
    const mod = await loadLang()
    await instance.loadLanguage(mod.default as never)
    loaded.add(name)
  }
  return instance.codeToHtml(code, {
    lang: name,
    themes: { light: "github-light", dark: "github-dark" },
    defaultColor: false,
  })
}
