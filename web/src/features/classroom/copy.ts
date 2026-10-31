/** Feature copy (sentence case, concrete verbs). */
export const COPY = {
  newChat: "New chat",
  chats: "Chats",
  noChats: "No chats yet.",
  startTitle: "Start learning something",
  startBody:
    "Name a topic. A short quiz finds where you stand, then a plan and a lesson follow.",
  startPlaceholder: "What do you want to learn?",
  jumpToLatest: "Jump to latest",
  stop: "Stop",
  send: "Send",
  steerOn: "Steer",
  waiting: "Waiting for an available model account",
  retry: "Retry",
  resume: "Resume",
  copyDiagnostics: "Copy diagnostics",
  compaction: "Earlier conversation summarized",
  interrupted: "Interrupted",
  questionUpdated: "Question updated",
  engineDown:
    "No model account is ready, so the tutor cannot answer. Check Accounts.",
} as const

const TOOL_LABELS: Record<string, string> = {
  probe: "Probe asked",
  recall: "Recall asked",
  quiz: "Quiz asked",
  explain: "Explain asked",
  ask_user_question: "Question asked",
  problem: "Problem set",
  plan_init: "Plan drafted",
  plan_update: "Plan updated",
  conclude: "Problem concluded",
  research: "Research started",
  pdf: "PDF reading started",
  visual: "Diagram started",
  demo: "Demo started",
}

export function toolLabel(name: string): string {
  return (
    TOOL_LABELS[name] ??
    name.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase())
  )
}

const TIME = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
})
const RELATIVE = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })

export function formatTime(epochMs: number): string {
  return TIME.format(new Date(epochMs))
}

/** "3 hours ago" style label for list rows; `now` is injected so tests stay deterministic. */
export function formatRelative(epochMs: number, now = Date.now()): string {
  const seconds = Math.round((epochMs - now) / 1000)
  const abs = Math.abs(seconds)
  if (abs < 45) return RELATIVE.format(0, "second")
  if (abs < 2700) return RELATIVE.format(Math.round(seconds / 60), "minute")
  if (abs < 79_200) return RELATIVE.format(Math.round(seconds / 3600), "hour")
  if (abs < 2_246_400)
    return RELATIVE.format(Math.round(seconds / 86_400), "day")
  return RELATIVE.format(Math.round(seconds / 2_592_000), "month")
}

const DURATION = new Intl.NumberFormat(undefined, { minimumIntegerDigits: 2 })

/** `m:ss` for elapsed or countdown labels. */
export function formatClock(totalSeconds: number): string {
  const s = Math.max(0, Math.floor(totalSeconds))
  return `${Math.floor(s / 60)}:${DURATION.format(s % 60)}`
}
