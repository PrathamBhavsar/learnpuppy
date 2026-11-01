import type { InfoContent } from "./info"

/** Plain-words explanations of the parts of the app frame (sidebar, top bar). */
export const SHELL_INFO = {
  workspaceChip: {
    title: "Workspace switcher",
    short: "The subject you are studying. Click to switch or create another.",
    body: [
      "A workspace is a separate notebook for one subject, for example Spanish. Chats, the knowledge map and review cards never leak between workspaces.",
      "The list shows each workspace with a number when it has flashcards due today. The Manage all link goes to the home screen where you can rename, archive, export or delete.",
    ],
    tips: [
      "Cmd/Ctrl plus 1 to 9 jumps straight to the first nine workspaces.",
      "Switching keeps you in the same section, so you stay on Review while you hop between subjects.",
    ],
  },
  search: {
    title: "Search and commands",
    short: "Find any screen, chat or action. Shortcut: Cmd/Ctrl plus K.",
    body: [
      "Opens the command palette. Type a few letters, pick a result with the arrow keys and press Enter. It can switch workspace, jump to a section or start common actions.",
    ],
  },
  accounts: {
    title: "Accounts pill",
    short: "How many of your AI logins are ready to answer right now.",
    body: [
      "LearnPuppy has no AI of its own. It uses the logins already on this computer for the Claude, Codex and Antigravity command-line tools. The pill reads, for example, 2/3 ready.",
      "Green means all are ready, amber means some are resting after hitting a usage limit (a countdown shows when the next one is back), and red means none can answer so chats and ingest are paused.",
    ],
    tips: ["Click it to open the Accounts screen."],
  },
  jobs: {
    title: "Running jobs",
    short:
      "Background work in this workspace, such as research or an ingest run.",
    body: [
      "Some tasks take a while, for example reading a PDF or importing your history. They run in the background and appear here with a small count badge. Jobs that can be stopped have a cancel button.",
    ],
  },
  theme: {
    title: "Theme",
    short: "Light, dark, or follow your system.",
    body: [
      "Changes the colours of the whole app. System follows your computer's setting.",
    ],
  },
  direction: {
    title: "Layout direction",
    short:
      "LTR is left to right. RTL mirrors the layout for Arabic, Hebrew and similar.",
    body: [
      "Flips the layout so it reads from right to left. Useful if you study a right-to-left language.",
    ],
  },
  help: {
    title: "Help",
    short: "Replay the welcome tour, explain this screen, or see shortcuts.",
    body: ["Everything that explains the app lives behind this button."],
  },
} satisfies Record<string, InfoContent>

/** The "what is everything on screen" guide opened from the Help menu. */
export const SHELL_GUIDE_PARTS: InfoContent[] = [
  SHELL_INFO.workspaceChip,
  {
    title: "Graph",
    short: "Your knowledge map.",
    body: [
      "Every idea you learn is a dot, joined to related ideas. Use it to see what you know and what is still missing.",
    ],
  },
  {
    title: "Learn",
    short: "Chat with your tutor.",
    body: [
      "Start a chat and the tutor explains, quizzes and checks your answers. A badge shows chats that are waiting for you.",
    ],
  },
  {
    title: "Review",
    short: "Flashcards at the right time.",
    body: [
      "Cards come back on a schedule designed to show them just before you forget. The badge is how many are due today.",
    ],
  },
  {
    title: "Digest",
    short: "A look back at what you studied.",
    body: ["Recent activity grouped by day, with a short quiz on it."],
  },
  {
    title: "Ingest",
    short: "Bring in what you already have.",
    body: [
      "Reads your past AI conversations and project history on this computer and turns them into knowledge and cards.",
    ],
  },
  {
    title: "Accounts and Settings (bottom of the sidebar)",
    short: "Which AI logins are used, and how the app behaves.",
    body: [
      "Accounts lists the logins found on this computer. Settings holds appearance, limits and your data folder. The Collapse button (or the [ key) shrinks the sidebar to icons; hover an icon for its name.",
    ],
  },
  SHELL_INFO.search,
  SHELL_INFO.jobs,
  SHELL_INFO.accounts,
  SHELL_INFO.direction,
  SHELL_INFO.theme,
]
