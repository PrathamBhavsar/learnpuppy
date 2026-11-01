/**
 * One stop of the welcome tour. `targets` are `data-tour` values; the first one found is highlighted.
 * A step that names targets is skipped when none is on screen; a step without targets is a text card.
 */
export type TourStep = {
  id: string
  title: string
  body: string[]
  targets?: string[]
  /** Hide the step unless this returns true (`has` checks for a `data-tour` element on screen). */
  show?: (has: (target: string) => boolean) => boolean
}

const inWorkspace = (has: (t: string) => boolean) => has("nav-graph")

export const TOUR_STEPS: readonly TourStep[] = [
  {
    id: "welcome",
    title: "Welcome to LearnPuppy",
    body: [
      "LearnPuppy is a private study app that runs on your own computer. You talk with an AI tutor, it keeps a map of everything you have learned, and it brings questions back just before you would forget.",
      "This short tour shows where things are. You can leave at any time and replay it from the Help button.",
    ],
  },
  {
    id: "workspaces",
    title: "Workspaces: one per subject",
    targets: ["workspace-chip", "home-new", "home-header"],
    body: [
      "A workspace is a separate notebook for one subject, for example Spanish. Its chats, knowledge map and review cards stay inside it, so subjects never get mixed up.",
      "Inside a workspace the chip in the sidebar shows where you are. Click it to switch, or press Cmd/Ctrl plus 1 to 9 to jump to the first nine.",
    ],
  },
  {
    id: "sections-overview",
    title: "What lives inside a workspace",
    show: (has) => !inWorkspace(has),
    body: [
      "Open a workspace and a sidebar appears with five sections: Graph (your knowledge map), Learn (chat with the tutor), Review (daily flashcards), Digest (what you studied lately) and Ingest (bring in your past work).",
      "Create or open a workspace to see them.",
    ],
  },
  {
    id: "graph",
    title: "Graph: your knowledge map",
    targets: ["nav-graph"],
    body: [
      "Everything you learn becomes a dot in a map, joined to related ideas. It grows as you study, so you can see what you know and what is still missing.",
    ],
  },
  {
    id: "learn",
    title: "Learn: talk to your tutor",
    targets: ["nav-learn"],
    body: [
      "Start a chat and the tutor explains, quizzes you and checks your answers. This is where most of the studying happens.",
    ],
  },
  {
    id: "review",
    title: "Review: flashcards at the right time",
    targets: ["nav-review"],
    body: [
      "Cards from your lessons come back on a schedule built to catch you just before you forget. The number on the badge is how many are due today.",
    ],
  },
  {
    id: "digest",
    title: "Digest: what you studied",
    targets: ["nav-digest"],
    body: [
      "A look back at recent activity and the decisions behind it, with a short quiz so it sticks.",
    ],
  },
  {
    id: "ingest",
    title: "Ingest: bring in what you already have",
    targets: ["nav-ingest"],
    body: [
      "Ingest reads your past AI conversations and project history on this computer and turns them into knowledge and cards. Your files stay on this machine; only what the AI needs to read is sent to it.",
    ],
  },
  {
    id: "search",
    title: "Search and jump anywhere",
    targets: ["search"],
    body: [
      "Press Cmd/Ctrl plus K to open the command palette. Type a few letters to find a screen, a chat or an action, then press Enter.",
    ],
  },
  {
    id: "accounts",
    title: "Accounts: which AI logins are used",
    targets: ["accounts"],
    body: [
      "LearnPuppy has no AI of its own. It borrows the logins you already use with the Claude, Codex and Antigravity command-line tools. This pill says how many are ready; if one hits a usage limit it rests for a while and another takes over.",
    ],
  },
  {
    id: "help",
    title: "Help and shortcuts",
    targets: ["help"],
    body: [
      "This button replays the tour and explains every part of the screen. Press the question mark key (?) anywhere to see all the keyboard shortcuts.",
    ],
  },
  {
    id: "appearance",
    title: "Make it comfortable",
    targets: ["appearance"],
    body: [
      "Switch between light, dark and system colours, or flip the layout for right-to-left languages such as Arabic. More options, like reduced motion and compact spacing, are in Settings.",
    ],
  },
  {
    id: "settings",
    title: "Settings",
    targets: ["nav-settings"],
    body: [
      "Appearance, how many AI calls may run at once, the data folder and a copy-able diagnostics report live here.",
    ],
  },
  {
    id: "done",
    title: "You are ready",
    body: [
      "Start your first lesson: open or create a workspace, choose Learn, and tell the tutor what you want to study, for example Spanish from zero.",
      "Look for the small (i) icons and the About this screen buttons whenever you want something explained.",
    ],
  },
]
