import {
  CopyIcon,
  GraduationCapIcon,
  MessageSquareIcon,
  Trash2Icon,
} from "lucide-react"

import { useGraphNode } from "@/api/queries/graph"
import {
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
} from "@/components/ui/context-menu"
import { toastError, toastSuccess } from "@/lib/toast"

import { copy } from "../copy"

type Props = {
  ws: string
  /** Node the menu was opened on. */
  nodeId: string | null
  text: string
  /** How many nodes the actions apply to (the selection when the node is part of it). */
  count: number
  onPractice: () => void
  onDelete: () => void
  onOpenChat: (chatId: string) => void
}

/** Right-click menu for a node: practice, copy, jump to the source chat, delete. */
export function GraphContextMenu({
  ws,
  nodeId,
  text,
  count,
  onPractice,
  onDelete,
  onOpenChat,
}: Props) {
  const detail = useGraphNode(ws, nodeId)
  const chatId = detail.data?.provenanceDetail.chatId ?? null
  return (
    <ContextMenuContent data-testid="graph-context-menu" className="w-52">
      <ContextMenuItem onSelect={onPractice} data-testid="graph-menu-practice">
        <GraduationCapIcon />
        {count > 1 ? `Practice ${count} selected` : copy.menu.practice}
      </ContextMenuItem>
      <ContextMenuItem
        onSelect={() =>
          void navigator.clipboard
            ?.writeText(text)
            .then(() => toastSuccess(copy.menu.copied))
            .catch((error: unknown) => toastError(error, "Could not copy"))
        }
        data-testid="graph-menu-copy"
      >
        <CopyIcon />
        {copy.menu.copy}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={!chatId}
        onSelect={() => chatId && onOpenChat(chatId)}
        data-testid="graph-menu-chat"
      >
        <MessageSquareIcon />
        {copy.menu.sourceChat}
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuItem
        variant="destructive"
        onSelect={onDelete}
        data-testid="graph-menu-delete"
      >
        <Trash2Icon />
        {copy.menu.delete}
      </ContextMenuItem>
    </ContextMenuContent>
  )
}
