import type { InfoContent } from "@/components/app/info/info"

/** Plain-words help for the Home, New/Edit workspace and Trash screens. */
export const HOME_INFO = {
  numbers: {
    title: "The numbers on a workspace card",
    short:
      "Nodes, chains and chats show how much is in the workspace; due is what to review today.",
    body: [
      "Nodes are the separate ideas in this workspace's knowledge map, for example a Spanish word or a rule. Chains are the reasoning links that join ideas together. Chats are your conversations with the tutor.",
      "A due badge counts flashcards that are ready to review today. Cards come back on a schedule built to catch you just before you forget them.",
    ],
  },
  due: {
    title: "Due today",
    short: "Flashcards waiting for you, across all workspaces.",
    body: [
      "The button at the top adds up the flashcards that are due in every active workspace and opens one combined review. It only appears when something is due.",
    ],
  },
  archived: {
    title: "Archived",
    short: "Show workspaces you have put away.",
    body: [
      "Archiving makes a workspace read-only and hides it from the sidebar. Nothing is deleted. Turn this switch on to see archived workspaces and restore them from the card menu. The switch only appears once you have archived something.",
    ],
  },
  trash: {
    title: "Trash",
    short: "Deleted workspaces wait here until you purge them.",
    body: [
      "Deleting a workspace moves it to the trash. From there you can restore it or purge it, and purging cannot be undone.",
    ],
  },
  import: {
    title: "Import",
    short: "Bring in a workspace from a .zip made by Export.",
    body: [
      "Use this to restore a backup or move a workspace from another computer. Choose the .zip file that the Export action produced.",
    ],
  },
  newWorkspace: {
    title: "New workspace",
    short: "Start a fresh notebook for a subject.",
    body: [
      "A workspace keeps one subject's chats, knowledge map and flashcards together. Create one per subject, for example Spanish.",
    ],
  },
  repair: {
    title: "Damaged workspaces: Repair and Reveal path",
    short: "A workspace whose settings file cannot be read.",
    body: [
      "If a workspace's workspace.json file is unreadable, the card is marked Damaged and cannot be opened. Repair rewrites minimal settings from the folder name so it opens again; your notes and cards are not touched. Reveal path copies the workspace's folder location so you can look at the files yourself.",
    ],
  },
  cardMenu: {
    title: "Card menu",
    short: "Rename, edit sources, export, archive or delete.",
    body: [
      "The three dots on a card hold the actions for that workspace. Rename and edit changes the name, icon, colour and description. Edit sources opens Ingest. Export saves a .zip backup. Archive puts it away; Delete moves it to the trash.",
    ],
  },
} satisfies Record<string, InfoContent>

export const HOME_GUIDE_PARTS: InfoContent[] = [
  {
    title: "Workspace cards",
    short: "One card per subject. Click a card to open it.",
    body: HOME_INFO.numbers.body,
  },
  HOME_INFO.cardMenu,
  HOME_INFO.due,
  HOME_INFO.archived,
  HOME_INFO.trash,
  HOME_INFO.import,
  HOME_INFO.newWorkspace,
  HOME_INFO.repair,
]

export const WORKSPACE_FIELD_INFO = {
  name: {
    title: "Workspace name",
    short:
      "What you call this subject. It must be different from your other workspaces.",
    body: ["Shown in the sidebar and on the home screen, for example Spanish."],
  },
  icon: {
    title: "Workspace icon",
    short: "A small picture that makes the workspace easy to spot.",
    body: ["Purely visual. Pick whichever you like; you can change it later."],
  },
  color: {
    title: "Workspace colour",
    short: "The accent colour of the workspace's icon.",
    body: ["Purely visual. It helps you tell workspaces apart at a glance."],
  },
  description: {
    title: "Workspace description",
    short: "A line or two about what this workspace is for.",
    body: [
      "Shown on the card on the home screen. It is only a reminder for you.",
    ],
  },
} satisfies Record<string, InfoContent>

export const TRASH_GUIDE_PARTS: InfoContent[] = [
  {
    title: "The list",
    short:
      "Each row is a deleted workspace with when it was deleted and how big it is.",
    body: [
      "Deleting a workspace never removes it straight away. It waits here so a mistake is easy to undo.",
    ],
  },
  {
    title: "Restore",
    short: "Puts the workspace back on the home screen with everything in it.",
  },
  {
    title: "Purge",
    short: "Deletes the workspace and its files for good.",
    body: ["You are asked to confirm. A purged workspace cannot be restored."],
  },
]
