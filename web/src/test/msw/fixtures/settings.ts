import type { SettingsCatalog, SettingsDoc } from "@/api/pending-types"

const ROLES = [
  "prober",
  "teacher",
  "practice",
  "extractor",
  "auditor",
  "pdf",
  "research",
  "visual",
  "demo",
] as const

export function settingsFixture(): SettingsDoc {
  const profiles: SettingsDoc["profiles"] = {}
  for (const role of ROLES) {
    profiles[role] = {
      provider: "claude",
      model: role === "teacher" ? "claude-opus-5-5" : "claude-sonnet-5-5",
      effort: role === "teacher" ? "high" : "medium",
      fallback: true,
    }
  }
  return {
    profiles,
    pins: { teacher: "claude:auto" },
    learningStyle: "default",
    concurrencyCap: 2,
    ui: {
      theme: "system",
      direction: "ltr",
      density: "comfortable",
      readingFont: "sans",
      reduceMotion: false,
      confirmDestructive: true,
      autoOpenBrowser: false,
    },
  }
}

export const CATALOG: SettingsCatalog = {
  providers: [
    {
      id: "claude",
      models: [
        "claude-fable-5-1",
        "claude-opus-5-5",
        "claude-sonnet-5-5",
        "claude-haiku-5-5",
      ],
      freeModels: [],
    },
    { id: "codex", models: ["gpt-5.5", "gpt-5.5-mini"], freeModels: [] },
    { id: "agy", models: ["gemini-3-pro", "gemini-3-flash"], freeModels: [] },
  ],
  efforts: ["low", "medium", "high", "xhigh"],
  roles: [...ROLES],
  pinRoles: ["teacher", "prober", "extractor"],
  themes: ["light", "dark", "system"],
  directions: ["ltr", "rtl", "auto"],
  densities: ["comfortable", "compact"],
}
