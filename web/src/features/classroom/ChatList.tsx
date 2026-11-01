import { useNavigate } from "@tanstack/react-router"
import { MessagesSquareIcon, PlusIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { ChatSummary } from "@/api/pending-types"
import { useChats } from "@/api/queries/chats"
import { ErrorState } from "@/components/app/error-state"
import { ScreenGuide } from "@/components/app/info/info"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"

import { DeleteChatDialog, RenameChatDialog } from "./ChatDialogs"
import { ChatListItem } from "./ChatListItem"
import { COPY } from "./copy"
import { classroomGuide } from "./info"
import { NewChatDialog } from "./NewChatDialog"

type ChatListProps = { ws: string; activeChatId?: string }

/** The 280 px chat column: new chat, then every chat with phase, kind, pending dot and age. */
export function ChatList({ ws, activeChatId }: ChatListProps) {
  const chats = useChats(ws)
  const navigate = useNavigate()
  const [creating, setCreating] = useState(false)
  const [renaming, setRenaming] = useState<ChatSummary | null>(null)
  const [deleting, setDeleting] = useState<ChatSummary | null>(null)

  return (
    <>
      <div className="flex flex-col gap-3">
        <PageHeader
          title="Learn"
          description="Teach and practice chats."
          actions={
            <Button
              size="sm"
              onClick={() => setCreating(true)}
              data-testid="new-chat"
              aria-label={COPY.newChat}
            >
              <PlusIcon data-icon="inline-start" />
              New
            </Button>
          }
        />
        <div className="self-start">
          <ScreenGuide {...classroomGuide} />
        </div>
        <nav aria-label={COPY.chats} data-testid="chat-list">
          {chats.isPending ? (
            <div
              role="status"
              aria-label="Loading"
              className="flex flex-col gap-2"
              data-testid="chat-list-loading"
            >
              {[0, 1, 2, 3].map((i) => (
                <SkeletonBlock key={i} className="h-14 w-full" />
              ))}
            </div>
          ) : chats.isError ? (
            <ErrorState
              title="Could not load chats"
              message={toApiError(chats.error).message}
              requestId={toApiError(chats.error).requestId}
              onRetry={() => chats.refetch()}
              className="px-0 py-6"
            />
          ) : chats.data.length === 0 ? (
            <div
              data-testid="chat-list-empty"
              className="flex flex-col items-center gap-2 py-8 text-center"
            >
              <MessagesSquareIcon
                aria-hidden
                className="size-6 text-muted-foreground"
                strokeWidth={1.5}
              />
              <p className="text-[13px] text-muted-foreground">
                {COPY.noChats}
              </p>
            </div>
          ) : (
            <ul className="flex flex-col gap-1" role="list">
              {chats.data.map((chat) => (
                <ChatListItem
                  key={chat.id}
                  ws={ws}
                  chat={chat}
                  active={chat.id === activeChatId}
                  onRename={setRenaming}
                  onDelete={setDeleting}
                />
              ))}
            </ul>
          )}
        </nav>
      </div>
      <NewChatDialog ws={ws} open={creating} onOpenChange={setCreating} />
      <RenameChatDialog
        ws={ws}
        chat={renaming}
        onClose={() => setRenaming(null)}
      />
      <DeleteChatDialog
        ws={ws}
        chat={deleting}
        onClose={() => setDeleting(null)}
        onDeleted={(chat) => {
          if (chat.id === activeChatId)
            void navigate({ to: "/w/$ws/learn", params: { ws } })
        }}
      />
    </>
  )
}
