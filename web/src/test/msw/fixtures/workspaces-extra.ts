import type { TrashItem, WorkspaceDetail } from "@/api/pending-types"

import { DAY } from "./rng"

/** A damaged workspace only exists when a test or the dev-sandbox keeps it in the db. */
export const WS_DAMAGED = "ws_recovered"

/** `workspace.json` is unreadable: the card offers Repair and Reveal path (docs/web/02). */
export function damagedWorkspace(now: number): WorkspaceDetail {
  return {
    id: WS_DAMAGED,
    name: "Recovered",
    slug: "recovered",
    icon: "terminal",
    hue: "rose",
    description: "",
    archived: false,
    damaged: true,
    createdAt: now - 60 * DAY,
    lastOpenedAt: now - 10 * DAY,
    sources: {
      claude: true,
      codex: true,
      agy: true,
      git: true,
      allow: [],
      sinceDays: 30,
    },
    overrides: { profiles: {} },
    stats: {
      nodes: 0,
      chains: 0,
      chats: 0,
      dueCards: 0,
      lastActivityAt: now - 3 * DAY,
    },
  }
}

/** Deleted workspaces as the trash page lists them: one to restore, one to purge. */
export function trashFixtures(now: number): TrashItem[] {
  return [
    {
      id: "trash_ws_old1",
      workspaceId: "ws_compilers",
      name: "Compilers course",
      deletedAt: now - 2 * DAY,
      sizeBytes: Math.round(4.6 * 1_048_576),
    },
    {
      id: "trash_ws_old2",
      workspaceId: "ws_scratch",
      name: "Scratch pad",
      deletedAt: now - 6 * DAY,
      sizeBytes: 1_800_000,
    },
  ]
}
