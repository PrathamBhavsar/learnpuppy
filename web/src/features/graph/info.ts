import type { InfoContent } from "@/components/app/info/info"

// Plain-words explanations for the Graph screen. Each entry feeds both its InfoTip and the screen guide.
export const graphInfo = {
  graph: {
    title: "The knowledge graph",
    short: "A map of everything the app has learned from you, so far.",
    body: [
      "Each dot is one idea, written as a single sentence. We call it a proposition. For Spanish this could be 'ser is for who you are, estar is for how you are right now'.",
      "A line joins two ideas when one leads to the other. A whole line of reasoning from premises to a conclusion is called a chain. Ideas that belong to the same topic sit close together.",
      "The map grows by itself as you learn in chats and as you bring in past work sessions.",
    ],
    tips: [
      "Drag to move around, scroll to zoom, click a dot for its details.",
      "Hold Shift and drag to select many dots at once.",
      "Right-click a dot for a quick menu.",
    ],
  },
  sources: {
    title: "Source filter",
    short: "Show only ideas that came from the chosen places.",
    body: [
      "Every idea remembers where it came from: a chat with a coding assistant (Claude, Codex, Antigravity), a Git commit, or a learning chat inside this app.",
      "Switch a source off to hide its ideas, switch it on to bring them back. The colour of each dot tells you its source.",
    ],
    tips: [
      "For a Spanish course, 'Chat' is the one you care about: those are your lessons here.",
    ],
  },
  cluster: {
    title: "Topic cluster",
    short: "Focus on one group of closely related ideas.",
    body: [
      "The app groups ideas that are tightly connected into clusters, each named after a representative idea.",
      "Pick one to hide everything else, or choose 'All clusters' to see the whole map again.",
    ],
  },
  minDegree: {
    title: "Minimum connections",
    short: "Hide ideas that have fewer links than this number.",
    body: [
      "A connection is a chain that touches an idea. Raising the number hides loosely connected ideas so the well-connected, central ones stand out.",
      "Set it back to 0 to show everything.",
    ],
  },
  clear: {
    title: "Clear filters",
    short: "Show every idea again.",
    body: [
      "Appears only when a source, cluster or connection filter is hiding something. It resets all three.",
    ],
  },
  search: {
    title: "Search",
    short: "Find ideas by the words they contain.",
    body: [
      "Matching ideas stay bright and the rest fade. The count shows how many matched.",
    ],
    tips: [
      "Press Enter to jump to the first match.",
      "Press Escape to clear what you typed.",
    ],
  },
  view: {
    title: "View options",
    short: "Outlines, labels, minimap, follow and the chain table.",
    body: [
      "Cluster outlines: draws a soft shape around each group of related ideas.",
      "Labels: Auto shows the text of ideas when you are zoomed in enough, Always shows all, Never hides them.",
      "Minimap: a small overview in the corner. Drag inside it to move the main view.",
      "Follow new propositions: when new ideas appear (for example while you chat), the view glides to them.",
      "Table: opens a list of all chains below the graph.",
    ],
  },
  counts: {
    title: "Counts",
    short: "How many ideas (nodes) and chains are showing.",
    body: [
      "When filters hide some ideas you see 'X of Y nodes', so you know something is hidden.",
    ],
  },
  legend: {
    title: "Legend",
    short: "What the colours and shapes of the dots mean.",
    body: [
      "Colour tells you the source of an idea. Shape tells you its kind: a claim is a solid square, a concept is outlined, a procedure has a hole, and a decision has a ring around it.",
      "A thin line between two ideas is a chain, running from the premise to the conclusion.",
    ],
    tips: ["Click the title to fold the legend away."],
  },
  drawer: {
    title: "Idea details",
    short: "Everything the app knows about one idea.",
    body: [
      "Opens when you click a dot. It shows when the idea was first seen, the original snippet it came from (provenance), the chains that lead to it and from it, and any flashcards made from it.",
      "Click an idea in the chain lists to hop to it.",
    ],
  },
  provenance: {
    title: "Provenance",
    short: "Where this idea came from.",
    body: [
      "The exact snippet of conversation that produced the idea. 'Open source chat' takes you back to that conversation, right where it happened.",
    ],
  },
  chains: {
    title: "Chains",
    short: "How ideas lead to one another.",
    body: [
      "'Supported by' lists the ideas that this one rests on (premises). 'Leads to' lists the ideas that follow from it (conclusions).",
      "Words like 'supports', 'implies' or 'contrasts' say how the two ideas relate.",
    ],
  },
  cards: {
    title: "Linked flashcards",
    short: "Review cards that test this idea.",
    body: [
      "Flashcards are made from ideas so you can remember them. The ring shows how many of this idea's cards have graduated into regular review, which is a rough measure of how well you know it.",
    ],
  },
  selection: {
    title: "Selection bar",
    short: "Actions for the ideas you have selected.",
    body: [
      "Select ideas by clicking, Shift-clicking or Shift-dragging a box. Then:",
      "Practice selected starts a practice chat that quizzes you on exactly those ideas.",
      "Delete removes them from the graph. You will see what else is affected first.",
    ],
    tips: ["Press Escape to clear the selection."],
  },
  delete: {
    title: "Delete safely",
    short: "Preview what a delete would remove before it happens.",
    body: [
      "Chains need both ends. When you delete an idea, every chain that touches it goes too. The dialog counts these first and asks you to tick a box if any would be lost.",
    ],
  },
  table: {
    title: "Chain table",
    short: "Every chain as a sortable list.",
    body: [
      "Shows each chain with its premises, its conclusion and how they relate. With ideas selected, only their chains are listed. Click a column name to sort, and use Export CSV to save the list.",
    ],
  },
  menu: {
    title: "Right-click menu",
    short: "Quick actions on one idea.",
    body: [
      "Right-click a dot to practise it, copy its text, jump to the chat it came from, or delete it.",
    ],
  },
} satisfies Record<string, InfoContent>

export const graphGuide = {
  title: "About the Graph",
  summary:
    "The Graph is your memory map: every idea you have learned or worked on, and how the ideas connect.",
  parts: [
    graphInfo.graph,
    graphInfo.search,
    graphInfo.sources,
    graphInfo.cluster,
    graphInfo.minDegree,
    graphInfo.view,
    graphInfo.counts,
    graphInfo.legend,
    graphInfo.drawer,
    graphInfo.provenance,
    graphInfo.chains,
    graphInfo.cards,
    graphInfo.selection,
    graphInfo.delete,
    graphInfo.table,
    graphInfo.menu,
  ] satisfies InfoContent[],
}
