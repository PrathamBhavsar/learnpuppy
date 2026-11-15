import type { Chapter } from "./api"

/** Strings in one place. Server text (titles, errors, questions) never goes through here: it is shown as it came. */
export const copy = {
  title: "Textbooks",
  description:
    "Upload a PDF to get its chapter list. Open a chapter to turn it into a short reading check, scheduled for review.",
  upload: "Upload a PDF",
  uploading: "Uploading",
  uploadHint:
    "PDF, up to 50 MB. Chapters are imported one at a time, when you open them.",
  tooBig: "That file is over 50 MB.",
  notPdf: "Choose a PDF file.",
  emptyTitle: "No textbooks yet",
  emptyBody:
    "Upload a PDF to get a chapter list, then open a chapter to get its reading check.",
  noChapters: "No chapters were found in this book.",
  existing: "This book is already in the workspace.",
  openHint: "Import a chapter when you are about to read it.",
} as const

/** The per-book limit on chapter imports in a rolling hour, as one sentence. */
export function hourLimitText(used: number, cap: number): string {
  return `${used} of ${cap} chapter imports used in the last hour. Try again a little later.`
}

const STATE_LABEL: Record<Chapter["state"], string> = {
  pending: "Not imported",
  importing: "Importing",
  ready: "Ready",
  failed: "Failed",
  skipped: "Skipped",
}

const STATUS_LABEL: Record<Chapter["status"], string> = {
  not_read: "Not read",
  checked: "Checked",
  due: "Due",
}

/** What the chapter row shows: reading status once imported, import state before that. */
export function chapterLabel(c: Chapter): string {
  return c.state === "ready" ? STATUS_LABEL[c.status] : STATE_LABEL[c.state]
}

/** The server sends a short code, never page text. Known codes read as sentences; others show as sent. */
export function errorText(code: string): string {
  switch (code) {
    case "parse":
      return "The chapter could not be turned into questions."
    case "timeout":
      return "The model took too long on this chapter."
    case "too_few_objectives":
      return "Too few questions could be made from this chapter."
    case "no_cards":
      return "No review cards could be made from this chapter."
    case "engine_unavailable":
      return "No model was available. Try again later."
    default:
      return code
  }
}

export function pageRange(c: Chapter): string {
  return c.startPage === c.endPage
    ? `Page ${c.startPage}`
    : `Pages ${c.startPage} to ${c.endPage}`
}
