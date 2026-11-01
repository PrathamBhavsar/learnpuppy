import type { InfoContent } from "@/components/app/info/info"

// Plain-words explanations for the Ingest screen. Each entry feeds both its InfoTip and the screen guide.
export const ingestInfo = {
  ingest: {
    title: "Ingest",
    short: "Brings your past work with AI assistants into the app.",
    body: [
      "Ingest reads the session logs that AI coding assistants keep on your computer, plus Git history, and picks out the decisions in them. Those decisions fill the Digest and the Graph, and become flashcards for Review.",
      "The logs are read from your own computer. To pick out decisions, the text is summarised by the AI model accounts you set up, with secrets such as keys removed first.",
      "If you only study a language here, you do not need Ingest. Learning chats already add to the Graph and Review by themselves.",
    ],
  },
  tools: {
    title: "Tools to read",
    short: "Which assistants and history to read.",
    body: [
      "Claude, Codex and Antigravity are AI coding assistants; each keeps a record of your conversations. Git is the version history of your code projects.",
      "Switch off any you do not use.",
    ],
  },
  since: {
    title: "Look back",
    short: "How far into the past to read.",
    body: [
      "Only sessions from this many days back are read. A short window is quick; 'All time' reads everything it can find, which can take a while the first time.",
      "Sessions that have not changed since an earlier run are skipped, so running again is quick.",
    ],
  },
  folders: {
    title: "Project folders",
    short: "The folders ingest is allowed to read from.",
    body: [
      "For privacy, ingest only reads sessions that belong to the project folders listed here. Anything else is ignored.",
      "Type a full path such as /Users/you/code/project and press Add, or use 'Detect projects' to pick from a list.",
      "If a folder is listed in another workspace too, you get a warning because its sessions would be read twice.",
    ],
    tips: ["Remove a folder with its X button. Save to keep your changes."],
  },
  detect: {
    title: "Detect projects",
    short: "Find project folders from your tools' history.",
    body: [
      "Looks at where your assistants have recorded sessions and lists those folders. Tick the ones that belong to this workspace and add them.",
    ],
  },
  save: {
    title: "Save sources",
    short: "Keep your changes to the settings above.",
    body: [
      "Changes are not used until you save them. Run ingest stays locked while there are unsaved changes, so you never run with settings you did not mean.",
    ],
  },
  run: {
    title: "Run ingest",
    short: "Start reading now.",
    body: [
      "Runs in the background and shows progress. It goes through four steps: scanning sessions, extracting decisions, auditing them for mistakes, and writing flashcards.",
      "You can keep using the app while it runs, or cancel it.",
    ],
    tips: [
      "The button stays disabled until at least one project folder is saved.",
    ],
  },
  progress: {
    title: "Run progress",
    short: "Which step ingest is on and how many sessions are done.",
    body: [
      "The label shows the current step. The counter shows sessions done out of the total found. Cancel stops the run.",
    ],
  },
  lastRun: {
    title: "Last run",
    short: "What the most recent run found.",
    body: [
      "Sessions is how many were read. New decisions and New cards show what was added to your Digest and Review.",
    ],
  },
  history: {
    title: "Run history",
    short: "Every earlier run, with details.",
    body: [
      "Open a run to see its report: sessions read, new decisions, new cards and how many sessions were skipped.",
      "If something went wrong, the reasons are listed with a suggestion. 'View new decisions' opens them in the Digest.",
    ],
  },
} satisfies Record<string, InfoContent>

export const ingestGuide = {
  title: "About Ingest",
  summary:
    "Ingest reads your past AI-assistant sessions and Git history on this computer and turns the decisions in them into Digest entries, Graph ideas and flashcards.",
  parts: [
    ingestInfo.ingest,
    ingestInfo.tools,
    ingestInfo.since,
    ingestInfo.folders,
    ingestInfo.detect,
    ingestInfo.save,
    ingestInfo.run,
    ingestInfo.progress,
    ingestInfo.lastRun,
    ingestInfo.history,
  ] satisfies InfoContent[],
}
