import type { InfoContent } from "@/components/app/info/info"

/** Plain-words help for Settings and Workspace settings. */
export const SETTINGS_INFO = {
  appearance: {
    title: "Appearance",
    short: "How the app looks: colours, reading direction, motion and spacing.",
    body: [
      "Theme picks light, dark or the same as your computer. Direction mirrors the layout for right-to-left languages such as Arabic. Reduce motion swaps movement for simple fades. Density Compact tightens the spacing so more fits on screen.",
    ],
  },
  behavior: {
    title: "Behavior",
    short: "Safety prompts and how much work the AI may do at once.",
    body: [
      "Confirm destructive actions asks before deleting things. The concurrency cap is how many AI requests may run at the same moment, from 1 to 6. A higher number is faster but uses your usage allowance sooner and can hit limits. Open the browser on start only matters when you launch the app with the learnpuppy serve command.",
    ],
  },
  shortcuts: {
    title: "Keyboard shortcuts",
    short:
      "Every key you can press, generated from what the app really listens to.",
    body: [
      "Press the question mark key (?) anywhere to see the same list in a sheet. Press g and then a letter to go to a screen, and Cmd/Ctrl plus K to search.",
    ],
  },
  data: {
    title: "Data and diagnostics",
    short:
      "Where your files live, and a report you can paste when asking for help.",
    body: [
      "The data directory is the folder on this computer that holds all your workspaces and settings. Nothing is stored in the cloud. Copy puts its path on your clipboard.",
      "Copy diagnostics builds a report with versions, the engine mode, account health and which AI is used for which job. Emails, tokens and keys are left out, so it is safe to paste into a bug report.",
    ],
    tips: [
      "The Trash button opens deleted workspaces, which you can restore or purge.",
    ],
  },
  help: {
    title: "Welcome tour",
    short: "A short guided walk through the app.",
    body: [
      "Shows the same tour you saw on first start. It points at the real buttons and explains each one. You can also start it from the Help button in the top bar.",
    ],
  },
  about: {
    title: "About",
    short: "Which version is running and which AI engine it uses.",
    body: [
      "Engine shows Real CLIs when the app talks to your signed-in Claude, Codex or Antigravity tools, or Scripted (faux) when it plays back canned answers for testing.",
    ],
  },
} satisfies Record<string, InfoContent>

export const SETTINGS_GUIDE_PARTS: InfoContent[] = [
  SETTINGS_INFO.appearance,
  SETTINGS_INFO.behavior,
  SETTINGS_INFO.shortcuts,
  SETTINGS_INFO.data,
  SETTINGS_INFO.help,
  SETTINGS_INFO.about,
]

export const WS_SETTINGS_INFO = {
  general: {
    title: "General",
    short: "The workspace's name, description, icon and colour.",
    body: [
      "Changes apply as soon as you press Save. The name must be unique among your workspaces.",
    ],
  },
  method: {
    title: "Study method",
    short: "The limits and defaults the learning method runs on.",
    body: [
      "A bout is one focused block. The defaults (90 minutes, a 2 hour gap, 270 minutes a day) come from the study research behind the method, and the evidence for some of those numbers is weak. Every one is yours to change, and Reset brings back the default.",
    ],
  },
  sources: {
    title: "Sources",
    short: "Which folders and tools this workspace may read when you ingest.",
    body: [
      "Ingest means reading your past work (AI conversations and project history on this computer) and turning it into knowledge and cards. This shows how many folders are allowed and which tools are read. Edit sources opens the Ingest screen where you change them.",
    ],
  },
  overrides: {
    title: "Model overrides",
    short: "Use a different AI setup for this workspace only.",
    body: [
      "Each job in the app (such as asking questions or teaching) has a slot: which AI provider, model and effort it uses. By default a workspace follows the global slots from the Accounts screen. Turn on an override for a job to give this workspace its own choice.",
    ],
  },
  danger: {
    title: "Archive and delete",
    short: "Put the workspace away, or move it to the trash.",
    body: [
      "Archive makes it read-only and hides it from the sidebar; nothing is deleted and you can restore it from the home screen. Delete moves it to the trash, where it can be restored or purged for good.",
    ],
  },
} satisfies Record<string, InfoContent>

export const WS_SETTINGS_GUIDE_PARTS: InfoContent[] = [
  WS_SETTINGS_INFO.general,
  WS_SETTINGS_INFO.sources,
  WS_SETTINGS_INFO.overrides,
  WS_SETTINGS_INFO.method,
  WS_SETTINGS_INFO.danger,
]
