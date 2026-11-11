import type { Chapter } from "./api"

/** Strings in one place. Server text (titles, errors, questions) never goes through here: it is shown as it came. */
export const copy = {
  title: "Textbooks",
  description:
    "Upload a PDF. Each chapter becomes a short reading check, scheduled for review.",
  upload: "Upload a PDF",
  uploading: "Uploading",
  uploadHint: "PDF, up to 50 MB. Drop a file here or choose one.",
  tooBig: "That file is over 50 MB.",
  notPdf: "Choose a PDF file.",
  emptyTitle: "No textbooks yet",
  emptyBody: "Upload a PDF to get a chapter list and a reading check for each.",
  noChapters: "No chapters were found in this book.",
  existing: "This book is already in the workspace.",
} as const

const STATE_LABEL: Record<Chapter["state"], string> = {
  pending: "Waiting",
  importing: "Importing",
  ready: "Ready",
  failed: "Failed",
  skipped: "Skipped",
  queued: "Queued",
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
    case "budget":
      return "The model-call budget for this book is used up."
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
