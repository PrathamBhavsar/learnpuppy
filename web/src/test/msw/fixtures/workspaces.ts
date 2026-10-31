import type { WorkspaceDetail, WorkspaceSources } from "@/api/pending-types"

import { DAY, HOUR } from "./rng"

export const WS_RUST = "ws_rustasync"
export const WS_DIST = "ws_distsys"
export const WS_DESIGN = "ws_designnotes"
const WS_ARCHIVED = "ws_oldnotes"

const sources = (allow: string[]): WorkspaceSources => ({
  claude: true,
  codex: true,
  agy: true,
  git: true,
  allow,
  sinceDays: 30,
})

export function workspaceFixtures(now: number): WorkspaceDetail[] {
  const base = {
    damaged: false,
    overrides: { profiles: {} },
  } as const
  return [
    {
      ...base,
      id: WS_RUST,
      name: "Rust async",
      slug: "rust-async",
      icon: "book-open",
      hue: "violet",
      description:
        "Futures, executors, pinning and the tokio runtime internals.",
      archived: false,
      createdAt: now - 40 * DAY,
      lastOpenedAt: now - 2 * HOUR,
      sources: sources([
        "/Users/dev/code/tokio-notes",
        "/Users/dev/code/learnpuppy",
      ]),
      stats: {
        nodes: 150,
        chains: 196,
        chats: 7,
        dueCards: 7,
        lastActivityAt: now - 2 * HOUR,
      },
    },
    {
      ...base,
      id: WS_DIST,
      name: "Distributed systems",
      slug: "distributed-systems",
      icon: "network",
      hue: "teal",
      description:
        "Consensus, replication and failure detectors from the course.",
      archived: false,
      createdAt: now - 21 * DAY,
      lastOpenedAt: now - 3 * DAY,
      sources: sources(["/Users/dev/code/raft-lab"]),
      stats: {
        nodes: 40,
        chains: 51,
        chats: 1,
        dueCards: 0,
        lastActivityAt: now - 3 * DAY,
      },
    },
    {
      ...base,
      id: WS_DESIGN,
      name: "Design notes",
      slug: "design-notes",
      icon: "palette",
      hue: "orange",
      description:
        "Typography, colour theory and layout decisions for the web app.",
      archived: false,
      createdAt: now - 9 * DAY,
      lastOpenedAt: now - 20 * 60_000,
      sources: sources([]),
      stats: {
        nodes: 28,
        chains: 33,
        chats: 1,
        dueCards: 12,
        lastActivityAt: now - 20 * 60_000,
      },
    },
    {
      ...base,
      id: WS_ARCHIVED,
      name: "Old notes",
      slug: "old-notes",
      icon: "archive",
      hue: "gold",
      description: "Archived scratch workspace.",
      archived: true,
      createdAt: now - 120 * DAY,
      lastOpenedAt: now - 90 * DAY,
      sources: sources([]),
      stats: {
        nodes: 12,
        chains: 9,
        chats: 0,
        dueCards: 0,
        lastActivityAt: now - 90 * DAY,
      },
    },
  ]
}
