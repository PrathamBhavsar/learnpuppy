import { expect, it } from "vitest"

import { jobDetail } from "./WorkspaceJobs"

it("shows PDF page progress and falls back to the kind", () => {
  expect(jobDetail({ kind: "pdf", pagesRead: 12, totalPages: 40 })).toBe(
    "pdf · 12/40 pages"
  )
  expect(jobDetail({ kind: "pdf", pagesRead: null, totalPages: 40 })).toBe(
    "pdf · 0/40 pages"
  )
  expect(jobDetail({ kind: "pdf" })).toBe("pdf")
  expect(jobDetail({ kind: "research", pagesRead: 1, totalPages: 2 })).toBe(
    "research"
  )
})
