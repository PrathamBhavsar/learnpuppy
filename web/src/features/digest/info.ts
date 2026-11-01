import type { InfoContent } from "@/components/app/info/info"

// Plain-words explanations for the Digest screen. Each entry feeds both its InfoTip and the screen guide.
export const digestInfo = {
  digest: {
    title: "Digest",
    short: "A daily log of the choices your AI coding assistants made.",
    body: [
      "When you work with AI coding assistants (Claude, Codex, Antigravity) or use Git, they make many small choices: which approach to take, which library to use, what to leave out. The digest collects these decisions, grouped by day and by working session.",
      "Each decision can also become flashcards, so you can learn from your own projects. If you only use this app to study a language, this screen will stay empty and you can skip it.",
    ],
    tips: ["Digest is filled by Ingest. Run Ingest first if it is empty."],
  },
  range: {
    title: "Date range",
    short: "Choose which days to show.",
    body: [
      "Today, the last 7 days, the last 30 days, or a custom start and end date.",
    ],
  },
  search: {
    title: "Search",
    short: "Find decisions by words in their title, project or summary.",
    body: ["Type a few words. Days and sessions without a match are hidden."],
  },
  sources: {
    title: "Sources",
    short: "Show only sessions from the tools you pick.",
    body: [
      "A source is where a session came from: Claude, Codex, Antigravity or Git commits. Switch one on to filter; switch all off to see everything.",
    ],
  },
  sessions: {
    title: "Days, sessions and decisions",
    short: "How the list is organised.",
    body: [
      "Each day is a group. Inside a day, a session is one stretch of work with one assistant on one project, with a short summary. Open a session to see the decisions in it.",
      "The numbers on the right of a session say how many decisions it holds and how many flashcards were made from them.",
    ],
    tips: [
      "Click a day to fold it away.",
      "Click a decision to see all its details.",
    ],
  },
  reviewThese: {
    title: "Review these",
    short: "Quiz yourself on the decisions you are looking at.",
    body: [
      "Opens Review with only the flashcards from the decisions currently shown. Use it after a filter, for example 'what did I decide this week?'.",
    ],
  },
  decision: {
    title: "A decision",
    short: "One choice, why it was made, and what else was possible.",
    body: [
      "Chosen is what was picked. Why is the reasoning. Risk is what could go wrong with it.",
      "Files lists the files it affected.",
    ],
  },
  basis: {
    title: "Stated or inferred",
    short: "How sure we are that this decision was really made.",
    body: [
      "Stated: the session said it outright, so we are confident. Inferred: the app worked it out from the code changes, so treat it as a good guess.",
    ],
  },
  alternatives: {
    title: "Alternatives considered",
    short: "Other options, and why each was rejected.",
    body: [
      "Knowing what was not chosen, and why, is often the best way to understand the choice.",
    ],
  },
  commits: {
    title: "Related commits",
    short: "Saved snapshots of code (Git commits) tied to this decision.",
    body: [
      "A commit is a saved version of a project's code. Linked commits show where the decision turned into real changes.",
    ],
  },
  cards: {
    title: "Generated cards",
    short: "Flashcards made from this decision.",
    body: [
      "Each decision can produce multiple-choice flashcards that fit into your normal Review. 'Open in card browser' jumps to a card.",
    ],
  },
  regenerate: {
    title: "Regenerate cards",
    short: "Make fresh flashcards for this decision.",
    body: [
      "Asks the AI to write the cards again, for example if the current ones are confusing. If a card is rejected as invalid, the reasons are shown below the button.",
    ],
  },
} satisfies Record<string, InfoContent>

export const digestGuide = {
  title: "About Digest",
  summary:
    "Digest shows what your AI assistants decided in your projects, day by day, and turns those decisions into review cards.",
  parts: [
    digestInfo.digest,
    digestInfo.range,
    digestInfo.search,
    digestInfo.sources,
    digestInfo.sessions,
    digestInfo.reviewThese,
    digestInfo.decision,
    digestInfo.basis,
    digestInfo.alternatives,
    digestInfo.commits,
    digestInfo.cards,
    digestInfo.regenerate,
  ] satisfies InfoContent[],
}
