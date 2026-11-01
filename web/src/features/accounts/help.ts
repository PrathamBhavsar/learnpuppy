import type { InfoContent } from "@/components/app/info/info"

/** Plain-words help for the Accounts screen. */
export const ACCOUNTS_INFO = {
  accounts: {
    title: "Accounts",
    short: "The AI logins LearnPuppy found on this computer.",
    body: [
      "LearnPuppy has no AI of its own. It looks for the Claude, Codex, Antigravity, Kiro and OpenCode command-line tools you are already signed in to and borrows them. Kiro and OpenCode have free options. Each login is one card, grouped by provider.",
      "Every login has its own usage allowance. Several logins let the app keep working when one runs out.",
    ],
  },
  status: {
    title: "Status",
    short: "Ready, cooling down, sign-in needed, or disabled.",
    body: [
      "Ready means it can answer now. Cooling down means it hit a usage limit and is resting; the app switches to another login meanwhile. Sign-in needed means the login expired, so run the command shown on the card in a terminal and then press Rescan. Disabled means you turned it off.",
    ],
  },
  cooldown: {
    title: "Cooldown ring",
    short: "A countdown until a resting login can be used again.",
    body: [
      "When a login hits its usage limit it pauses. The ring empties as the wait runs down and the time left is shown. The account becomes ready by itself when it reaches zero.",
    ],
  },
  toggle: {
    title: "Enable or disable",
    short: "Stop the app using a login without signing out.",
    body: [
      "Switch a card off to keep that login out of rotation, for example to save its allowance. Switch it on again any time.",
    ],
  },
  usage: {
    title: "Requests and tokens",
    short: "How much this login has been used by LearnPuppy.",
    body: [
      "Requests counts calls made. Tokens in and out are the amount of text sent to and received from the AI, measured in small pieces of words called tokens.",
    ],
  },
  rescan: {
    title: "Rescan",
    short: "Look for logins again.",
    body: [
      "Use it after you sign in to a tool in a terminal, or after a login expires and you renewed it.",
    ],
  },
  addProfile: {
    title: "Add Claude profile",
    short: "Add another Claude login stored in a different folder.",
    body: [
      "If you keep a second Claude login in its own folder (the one you would set as CLAUDE_CONFIG_DIR), enter that folder here. It then counts as a separate allowance.",
    ],
  },
  freeModels: {
    title: "Use free models",
    short: "Put every role on a free model in one click.",
    body: [
      "Sets each role to a free OpenCode model or a Kiro model, and lets a role move to the next free model when one is limited. It never picks a paid model. Your learning style and other settings stay as they are.",
    ],
  },
  roleSlots: {
    title: "Role slots",
    short: "Which AI, model and effort each job in the app uses.",
    body: [
      "The app gives different jobs to different roles, for example asking questions and teaching. For each role you choose a provider, a model and an effort (how hard it thinks; higher is slower but more careful). Fallback lets a role use other providers when its first choice is resting.",
      "A workspace can override these in its own settings.",
    ],
  },
} satisfies Record<string, InfoContent>

export const ACCOUNTS_GUIDE_PARTS: InfoContent[] = [
  ACCOUNTS_INFO.accounts,
  ACCOUNTS_INFO.status,
  ACCOUNTS_INFO.cooldown,
  ACCOUNTS_INFO.toggle,
  ACCOUNTS_INFO.usage,
  ACCOUNTS_INFO.rescan,
  ACCOUNTS_INFO.addProfile,
  ACCOUNTS_INFO.freeModels,
  ACCOUNTS_INFO.roleSlots,
]
