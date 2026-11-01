import { Link } from "@tanstack/react-router"
import {
  PanelRightCloseIcon,
  PanelRightOpenIcon,
  SearchXIcon,
} from "lucide-react"
import { useRef, useState } from "react"

import { ApiError, toApiError } from "@/api/errors"
import { newClientId } from "@/api/client"
import type {
  AnswerInput,
  ClassroomJob,
  ClassroomView,
} from "@/api/pending-types"
import {
  useCancelTurn,
  useChat,
  useSendTurn,
  useUploadPdf,
} from "@/api/queries/chats"
import { InfoTip, ScreenGuide } from "@/components/app/info/info"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { useStreamingTurn } from "@/hooks/use-streaming-turn"
import { storage } from "@/lib/storage"
import { toastError, toastInfo, toastSuccess } from "@/lib/toast"
import { useStatus } from "@/stores/status"

import { ArtifactViewer } from "./ArtifactViewer"
import { Composer } from "./Composer"
import { COPY } from "./copy"
import { classroomGuide, classroomInfo } from "./info"
import { DraftScope } from "./dock/drafts"
import { QuestionDock } from "./dock/QuestionDock"
import { MessageList } from "./MessageList"
import { PlanPanel } from "./PlanPanel"
import { ProblemResultCard } from "./ProblemResultCard"
import { ResizeHandle } from "./ResizeHandle"
import { RateLimitBanner, TurnErrorCard } from "./TurnStatus"
import "./styles.css"

const WIDTH_KEY = "lp-classroom-plan-width"
const OPEN_KEY = "lp-classroom-plan-open"
const MIN_W = 260
const MAX_W = 560
const DEFAULT_W = 340

function storedWidth(): number {
  const raw = Number(storage.get(WIDTH_KEY))
  return Number.isFinite(raw) && raw >= MIN_W && raw <= MAX_W ? raw : DEFAULT_W
}

function ClassroomSkeleton() {
  return (
    <div
      role="status"
      aria-label="Loading"
      data-testid="classroom-loading"
      className="flex size-full flex-col gap-4 p-4"
    >
      <SkeletonBlock className="h-6 w-1/3" />
      <SkeletonBlock className="ms-auto h-12 w-1/2" />
      <SkeletonBlock className="h-24 w-4/5" />
      <SkeletonBlock className="ms-auto h-10 w-2/5" />
      <div className="mt-auto">
        <SkeletonBlock className="h-24 w-full" />
      </div>
    </div>
  )
}

type ClassroomProps = { ws: string; chatId: string }

/** `/learn/:chat`: loads the server-rendered `ClassroomView` and keeps it fresh from SSE. */
export function Classroom({ ws, chatId }: ClassroomProps) {
  const query = useChat(ws, chatId)
  if (query.isPending) return <ClassroomSkeleton />
  if (query.isError) {
    const error = toApiError(query.error)
    if (error.code === "not_found")
      return (
        <EmptyState
          icon={SearchXIcon}
          title="Chat not found"
          description="It may have been deleted. Pick another chat or start a new one."
          action={
            <Button asChild variant="outline">
              <Link to="/w/$ws/learn" params={{ ws }}>
                Back to chats
              </Link>
            </Button>
          }
          className="h-full"
        />
      )
    return (
      <ErrorState
        title="Could not open this chat"
        message={error.message}
        requestId={error.requestId}
        onRetry={() => query.refetch()}
        className="h-full"
      />
    )
  }
  return (
    <ClassroomBody
      key={JSON.stringify([ws, chatId])}
      ws={ws}
      chatId={chatId}
      view={query.data}
    />
  )
}

function ClassroomBody({
  ws,
  chatId,
  view,
}: ClassroomProps & { view: ClassroomView }) {
  const send = useSendTurn(ws, chatId)
  const cancel = useCancelTurn(ws, chatId)
  const upload = useUploadPdf(ws, chatId)
  const live = useStreamingTurn(chatId)
  const engineDown = useStatus((s) => s.engineUnavailable)
  const [artifact, setArtifact] = useState<ClassroomJob | null>(null)
  const [width, setWidthState] = useState(storedWidth)
  const [open, setOpenState] = useState(() => storage.get(OPEN_KEY) !== "0")
  const splitRef = useRef<HTMLDivElement>(null)

  const setWidth = (next: number) => {
    setWidthState(next)
    storage.set(WIDTH_KEY, String(next))
  }
  const setOpen = (next: boolean) => {
    setOpenState(next)
    storage.set(OPEN_KEY, next ? "1" : "0")
  }

  const { turn, pending } = view
  const running = turn.state === "running"
  const error =
    turn.state === "error"
      ? (turn.error ?? {
          code: live?.error?.code ?? "internal",
          message: live?.error?.message ?? "",
          retryable: live?.error?.retryable ?? true,
          interrupted: false,
        })
      : null

  const post = (
    input: Parameters<typeof send.mutate>[0]["input"],
    clientId?: string
  ) =>
    send
      .mutateAsync(
        { input, clientId: clientId ?? newClientId() },
        {
          onError: (e) => {
            if (e instanceof ApiError && e.code === "stale_question")
              toastInfo(COPY.questionUpdated)
            else toastError(e, "Could not send")
          },
        }
      )
      .then(() => undefined)

  const answer = (value: AnswerInput) => {
    if (pending)
      return post({ type: "answer", callId: pending.callId, answer: value })
  }

  const attach = (file: File) =>
    upload.mutate(file, {
      onSuccess: () => toastSuccess("PDF uploaded. Reading it now."),
      onError: (e) => toastError(e, "Could not upload the PDF"),
    })

  return (
    <div
      ref={splitRef}
      data-testid="classroom"
      data-chat-id={chatId}
      className="relative flex size-full min-h-0"
    >
      <section
        aria-label="Conversation"
        className="flex min-w-0 flex-1 flex-col"
      >
        <header className="relative z-30 flex items-center gap-2 border-b bg-background px-4 py-2">
          <h2
            className="min-w-0 flex-1 truncate font-heading text-[14px] font-medium"
            title={view.title}
            data-testid="classroom-title"
          >
            {view.title}
          </h2>
          <Badge variant="secondary">
            {view.kind === "practice" ? "Practice" : "Teach"}
          </Badge>
          <InfoTip info={classroomInfo.kind} />
          <ScreenGuide {...classroomGuide} />
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={() => setOpen(!open)}
            aria-label={open ? "Hide plan panel" : "Show plan panel"}
            aria-expanded={open}
            data-testid="plan-toggle"
          >
            {open ? (
              <PanelRightCloseIcon className="rtl:-scale-x-100" />
            ) : (
              <PanelRightOpenIcon className="rtl:-scale-x-100" />
            )}
          </Button>
        </header>

        {view.messages.length === 0 && !running && !pending ? (
          <div
            data-testid="classroom-empty"
            className="flex min-h-0 flex-1 items-center justify-center text-[13px] text-muted-foreground"
          >
            Nothing here yet. Say hello to get started.
          </div>
        ) : (
          <MessageList chatId={chatId} view={view} />
        )}

        {view.problemResult && !running ? (
          <div className="mx-auto w-full max-w-3xl px-4 pb-3">
            <ProblemResultCard result={view.problemResult} />
          </div>
        ) : null}

        {engineDown ? (
          <p
            role="status"
            data-testid="engine-down"
            className="mx-auto w-full max-w-3xl border-t border-warning/40 bg-warning/10 px-4 py-2 text-[13px]"
          >
            {COPY.engineDown}{" "}
            <Link
              to="/accounts"
              className="text-primary underline underline-offset-2"
            >
              Open Accounts
            </Link>
          </p>
        ) : null}
        {turn.state === "queued" ? (
          <RateLimitBanner
            turn={turn}
            onCancel={() => cancel.mutate()}
            cancelling={cancel.isPending}
          />
        ) : null}
        {error ? (
          <TurnErrorCard
            chatId={chatId}
            turn={{ ...turn, error }}
            viewVersion={view.viewVersion}
            retrying={send.isPending}
            onRetry={() => {
              void post({ type: "resume" }, turn.clientId ?? undefined).catch(
                () => undefined
              )
            }}
          />
        ) : null}

        <DraftScope ws={ws} chatId={chatId}>
          {pending ? (
            <QuestionDock
              key={pending.callId}
              pending={pending}
              probe={view.probeProgress}
              busy={send.isPending || running}
              onSubmit={answer}
            />
          ) : (
            <Composer
              running={running || turn.state === "queued"}
              sending={send.isPending}
              stopping={cancel.isPending}
              attaching={upload.isPending}
              onSend={(text) => post({ type: "user", text })}
              onSteer={(text) => post({ type: "steer", text })}
              onStop={() => cancel.mutate()}
              onAttach={attach}
            />
          )}
        </DraftScope>
      </section>

      {open ? (
        <>
          <ResizeHandle
            width={width}
            min={MIN_W}
            max={MAX_W}
            onChange={setWidth}
            containerRef={splitRef}
          />
          <aside
            aria-label="Plan"
            style={{ width }}
            className="absolute inset-y-0 top-[3.25rem] end-0 z-20 w-full max-w-full shrink-0 border-s bg-background md:static md:top-auto md:z-auto"
          >
            <PlanPanel ws={ws} view={view} onOpenArtifact={setArtifact} />
          </aside>
        </>
      ) : null}

      <ArtifactViewer
        ws={ws}
        job={artifact}
        onClose={() => setArtifact(null)}
      />
    </div>
  )
}
