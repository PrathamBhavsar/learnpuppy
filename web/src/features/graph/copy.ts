// All user-visible strings for the graph screen live here (docs/web/04 §Conventions).
const counts = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 })
const whole = new Intl.NumberFormat()
const plural = new Intl.PluralRules()

export const formatCount = (n: number): string => (n >= 10_000 ? counts.format(n) : whole.format(n))

function pluralize(n: number, one: string, other: string): string {
  return `${formatCount(n)} ${plural.select(n) === "one" ? one : other}`
}

export const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" })

export const copy = {
  title: "Graph",
  canvasLabel: (nodes: number) =>
    `Knowledge graph with ${pluralize(nodes, "proposition", "propositions")}`,
  canvasUnsupported:
    "This browser can't draw the graph. Use the proposition list and the chain table instead.",
  canvasHelp:
    "Arrow keys move between propositions. Tab walks them by connection count. Enter opens details, Escape clears the selection, F fits the view, plus and minus zoom, Delete removes the selection.",
  search: "Search propositions",
  searchPlaceholder: "Search propositions",
  searchClear: "Clear search",
  searchMatches: (n: number) => (n === 0 ? "No matches" : pluralize(n, "match", "matches")),
  sources: "Source",
  allClusters: "All clusters",
  cluster: "Cluster",
  minDegree: "Min connections",
  hulls: "Cluster outlines",
  labels: "Labels",
  labelsAuto: "Auto",
  labelsAlways: "Always",
  labelsNever: "Never",
  minimap: "Minimap",
  followNew: "Follow new propositions",
  table: "Chain table",
  counts: (nodes: number, chains: number, total?: number) =>
    total !== undefined && total !== nodes
      ? `${formatCount(nodes)} of ${pluralize(total, "node", "nodes")} · ${pluralize(chains, "chain", "chains")}`
      : `${pluralize(nodes, "node", "nodes")} · ${pluralize(chains, "chain", "chains")}`,
  zoomIn: "Zoom in",
  zoomOut: "Zoom out",
  fit: "Fit to view",
  relayout: "Recompute layout",
  relayoutDone: "Graph layout recomputed",
  relayoutFailed: "Could not recompute the layout",
  clearFilters: "Clear filters",
  performanceOn: (n: number) => `Performance mode · ${formatCount(n)} nodes`,
  performanceOff: "Use full detail",
  performanceLarge: (n: number) => `Large graph · ${formatCount(n)} nodes`,
  performanceOnAction: "Performance mode",
  performanceHint: "Edge fading and labels are off.",
  empty: {
    title: "No propositions yet",
    description:
      "The graph is built from learning chats and ingested sessions. Teach something, or ingest your coding sessions to fill it.",
    learn: "Start learning",
    ingest: "Run ingest",
  },
  pending: {
    title: "Computing layout…",
    description: "Placing propositions so related ones sit together. This takes a moment on large graphs.",
  },
  error: { title: "Could not load the graph", message: "The graph snapshot failed to load." },
  noMatchesFilter: {
    title: "Nothing matches these filters",
    description: "Relax the source, cluster or connection filters to see propositions again.",
  },
  legend: {
    title: "Legend",
    sources: "Source",
    kinds: "Kind",
    claim: "Claim",
    concept: "Concept",
    procedure: "Procedure",
    decision: "Decision",
    chain: "Chain, from premise to conclusion",
    hull: "Cluster outline",
    sourceNames: { claude: "Claude", codex: "Codex", agy: "Antigravity", git: "Git", chat: "Chat" } as Record<string, string>,
  },
  drawer: {
    label: "Proposition details",
    close: "Close details",
    provenance: "Provenance",
    source: "Source",
    kind: "Kind",
    born: "First seen",
    session: "Session",
    openChat: "Open source chat",
    incoming: "Supported by",
    outgoing: "Leads to",
    noChains: "No chains yet",
    cards: "Linked flashcards",
    noCards: "No flashcards yet",
    mastery: "Mastery",
    practice: "Practice this",
    delete: "Delete…",
    loadFailed: "Could not load details",
    chainKinds: { supports: "supports", implies: "implies", contrasts: "contrasts", part_of: "part of" } as Record<string, string>,
    cardStates: { new: "New", learning: "Learning", review: "Review", relearning: "Relearning" } as Record<string, string>,
  },
  selection: {
    count: (n: number) => `${formatCount(n)} selected`,
    practice: "Practice selected",
    delete: "Delete…",
    clear: "Clear selection",
    practiceFailed: "Could not start a practice chat",
  },
  del: {
    title: (n: number) => `Delete ${pluralize(n, "proposition", "propositions")}?`,
    description: "Deleted propositions move out of the graph. Chains that lose an endpoint are removed with them.",
    loading: "Checking what this affects…",
    orphaned: (n: number) => `${pluralize(n, "chain loses", "chains lose")} an endpoint`,
    none: "No chains are affected.",
    more: (n: number) => `and ${formatCount(n)} more`,
    understand: "I understand these chains will be removed",
    confirm: "Delete",
    cancel: "Cancel",
    working: "Deleting…",
    done: (n: number) => `Deleted ${pluralize(n, "proposition", "propositions")}`,
    failed: "Could not delete",
    previewFailed: "Could not check what deleting affects",
    stale: "The graph changed since this preview. Review it again.",
  },
  table_: {
    label: "Chains",
    groups: "Group",
    chains: "Chains",
    kind: "Kind",
    from: "From",
    to: "To",
    export: "Export CSV",
    empty: "No chains to show",
    emptySelection: "No chains involve the selected propositions",
    premise: "Premise",
    conclusion: "Conclusion",
    expand: "Expand",
    collapse: "Collapse",
    resize: "Resize chain table",
    loadFailed: "Could not load chains",
    scope: (n: number) => (n === 0 ? "All chains" : `Chains of ${pluralize(n, "selected proposition", "selected propositions")}`),
    csvName: "graph-chains.csv",
  },
  menu: {
    practice: "Practice this",
    copy: "Copy text",
    copied: "Copied",
    sourceChat: "Show source chat",
    delete: "Delete…",
  },
  list: {
    label: "Propositions, most connected first",
    announceSelected: (text: string, degree: number) =>
      `Selected: ${text}. ${pluralize(degree, "connection", "connections")}.`,
    announceCleared: "Selection cleared",
  },
  minimapLabel: "Minimap. Drag to move the view.",
} as const
