import type { NodeSource } from "@/api/pending-types"

/** Token-backed classes per node source (chat has no hue of its own and uses the accent). */
export const SOURCE_BG: Record<NodeSource, string> = {
  claude: "bg-source-claude",
  codex: "bg-source-codex",
  agy: "bg-source-agy",
  git: "bg-source-git",
  chat: "bg-primary",
}
