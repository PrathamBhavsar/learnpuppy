import { Link } from "@tanstack/react-router"
import {
  DownloadIcon,
  MoreHorizontalIcon,
  PencilIcon,
  Trash2Icon,
} from "lucide-react"

import type { ChatPhase, ChatSummary } from "@/api/pending-types"
import { fetchChatExport } from "@/api/queries/chats"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { toastError } from "@/lib/toast"
import { cn } from "@/lib/utils"

import { formatRelative } from "./copy"

const RANK: Record<ChatPhase, number> = {
  probe: 0,
  plan: 1,
  teach: 2,
  practice: 2,
  done: 3,
}
const PHASE_LABEL: Record<ChatPhase, string> = {
  probe: "Probe",
  plan: "Plan",
  teach: "Teach",
  practice: "Practice",
  done: "Done",
}

function PhaseDots({ phase }: { phase: ChatPhase }) {
  const rank = RANK[phase]
  return (
    <span
      role="img"
      aria-label={`Phase: ${PHASE_LABEL[phase]}`}
      data-testid="chat-phase"
      className="flex items-center gap-0.5"
    >
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className={cn(
            "h-1 w-3",
            i < rank ? "bg-success" : i === rank ? "bg-primary" : "bg-muted"
          )}
        />
      ))}
    </span>
  )
}

async function exportChat(ws: string, chat: ChatSummary): Promise<void> {
  try {
    const markdown = await fetchChatExport(ws, chat.id)
    const url = URL.createObjectURL(
      new Blob([markdown], { type: "text/markdown" })
    )
    const link = document.createElement("a")
    link.href = url
    link.download = `${chat.title.replace(/[^\w.-]+/g, "-").slice(0, 60) || "chat"}.md`
    link.click()
    URL.revokeObjectURL(url)
  } catch (error) {
    toastError(error, "Could not export the chat")
  }
}

type ChatListItemProps = {
  ws: string
  chat: ChatSummary
  active: boolean
  onRename: (chat: ChatSummary) => void
  onDelete: (chat: ChatSummary) => void
}

export function ChatListItem({
  ws,
  chat,
  active,
  onRename,
  onDelete,
}: ChatListItemProps) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <li
          data-testid={`chat-item-${chat.id}`}
          data-active={active}
          className={cn(
            "group/chat relative border",
            active
              ? "border-primary/60 bg-primary/10"
              : "border-transparent hover:bg-muted/60"
          )}
        >
          <Link
            to="/w/$ws/learn/$chat"
            params={{ ws, chat: chat.id }}
            aria-current={active ? "page" : undefined}
            className="flex flex-col gap-1.5 px-2.5 py-2 pe-9 outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
          >
            <span className="flex items-center gap-1.5">
              {chat.pending ? (
                <span
                  role="img"
                  aria-label="Waiting for your answer"
                  data-testid="chat-pending-dot"
                  className="size-1.5 shrink-0 bg-primary"
                />
              ) : null}
              <span
                dir="auto"
                className="min-w-0 flex-1 truncate text-[13px] font-medium"
              >
                {chat.title}
              </span>
            </span>
            <span className="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted-foreground">
              <Badge variant="outline" className="h-4 px-1.5 font-normal">
                {chat.kind === "practice" ? "Practice" : "Teach"}
              </Badge>
              <PhaseDots phase={chat.phase} />
              {chat.jobs > 0 ? (
                <span className="whitespace-nowrap tabular-nums">
                  {chat.jobs} running
                </span>
              ) : null}
              <span className="ms-auto whitespace-nowrap">
                {formatRelative(chat.updatedAt)}
              </span>
            </span>
          </Link>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label={`Actions for ${chat.title}`}
                data-testid={`chat-menu-${chat.id}`}
                className="absolute end-1 top-1.5 opacity-0 group-focus-within/chat:opacity-100 group-hover/chat:opacity-100 data-[state=open]:opacity-100"
              >
                <MoreHorizontalIcon />
              </Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end">
              <DropdownMenuItem onSelect={() => onRename(chat)}>
                <PencilIcon /> Rename
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => void exportChat(ws, chat)}>
                <DownloadIcon /> Export as Markdown
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                variant="destructive"
                onSelect={() => onDelete(chat)}
              >
                <Trash2Icon /> Delete…
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </li>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => onRename(chat)}>
          <PencilIcon /> Rename
        </ContextMenuItem>
        <ContextMenuItem onSelect={() => void exportChat(ws, chat)}>
          <DownloadIcon /> Export as Markdown
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem variant="destructive" onSelect={() => onDelete(chat)}>
          <Trash2Icon /> Delete…
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}
