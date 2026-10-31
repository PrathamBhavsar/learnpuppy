import { DownloadIcon } from "lucide-react"
import { useMemo, useState } from "react"

import { toApiError } from "@/api/errors"
import type { IngestSourcesBody } from "@/api/pending-types"
import {
  useCancelIngest,
  useIngestRuns,
  useIngestSources,
  useRunIngest,
  useSaveIngestSources,
} from "@/api/queries/ingest"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"
import { toastError, toastSuccess } from "@/lib/toast"

import { RunHistory } from "./RunHistory"
import { RunPanel } from "./RunPanel"
import { SourcesEditor } from "./SourcesEditor"
import { isRunning, sourcesEqual } from "./model"

function pathErrorsFrom(error: unknown): Record<number, string> {
  const out: Record<number, string> = {}
  for (const field of toApiError(error).fields) {
    const match = /^allow\[(\d+)\]$/.exec(field.path)
    if (match?.[1]) out[Number(match[1])] = field.message
  }
  return out
}

export function IngestScreen({ ws }: { ws: string }) {
  usePageTitle("Ingest", useWorkspaceName(ws))
  const sources = useIngestSources(ws)
  const runs = useIngestRuns(ws)
  const save = useSaveIngestSources(ws)
  const run = useRunIngest(ws)
  const cancel = useCancelIngest(ws)

  const server = sources.data
  const [draft, setDraft] = useState<IngestSourcesBody | null>(null)
  const [pathErrors, setPathErrors] = useState<Record<number, string>>({})
  const [startedId, setStartedId] = useState<string | null>(null)

  const savedBody: IngestSourcesBody | null = useMemo(
    () =>
      server
        ? {
            claude: server.claude,
            codex: server.codex,
            agy: server.agy,
            git: server.git,
            allow: server.allow,
            sinceDays: server.sinceDays,
          }
        : null,
    [server]
  )

  // Adopt the server copy on first load and whenever it changes under an untouched draft.
  const [adopted, setAdopted] = useState(savedBody)
  if (savedBody !== adopted) {
    setAdopted(savedBody)
    if (savedBody && (!draft || (adopted && sourcesEqual(adopted, draft))))
      setDraft(savedBody)
  }
  if (savedBody && !draft) setDraft(savedBody)

  const dirty = Boolean(draft && savedBody && !sourcesEqual(draft, savedBody))

  const activeRun = runs.data?.find(isRunning)
  const finishedStarted =
    startedId !== null &&
    runs.data?.some((r) => r.id === startedId && !isRunning(r))
  const runId =
    server?.runningId ?? activeRun?.id ?? (finishedStarted ? null : startedId)

  const onSave = () => {
    if (!draft) return
    setPathErrors({})
    save.mutate(draft, {
      onSuccess: () => toastSuccess("Sources saved"),
      onError: (error) => {
        const errors = pathErrorsFrom(error)
        if (Object.keys(errors).length > 0) setPathErrors(errors)
        else toastError(error, "Could not save sources")
      },
    })
  }

  const onRun = () =>
    run.mutate(undefined, {
      onSuccess: ({ runId: id }) => setStartedId(id),
      onError: (error) => toastError(error, "Could not start ingest"),
    })

  const blockedReason = !server
    ? null
    : !server.enabled
      ? "Add a project folder and save the sources to enable ingest."
      : dirty
        ? "Save your source changes before running."
        : null

  const lastFinished = runs.data?.find((r) => !isRunning(r))
  const loading = sources.isPending || runs.isPending
  const failed = sources.isError ? sources : runs.isError ? runs : null

  return (
    <section
      data-testid="screen-ingest"
      aria-busy={loading}
      className="mx-auto flex min-h-full w-full max-w-5xl flex-col gap-6 p-6"
    >
      <PageHeader
        title="Ingest"
        description="Read agent sessions and commits for this workspace and turn them into decisions and review cards."
      />

      {loading ? (
        <div
          data-testid="ingest-loading"
          className="grid gap-4 lg:grid-cols-[1fr_22rem]"
        >
          <SkeletonBlock className="h-96 w-full" />
          <SkeletonBlock className="h-56 w-full" />
        </div>
      ) : failed ? (
        <ErrorState
          title="Ingest did not load"
          message={toApiError(failed.error).message}
          requestId={toApiError(failed.error).requestId}
          onRetry={() => Promise.all([sources.refetch(), runs.refetch()])}
          className="border"
        />
      ) : server && draft && runs.data ? (
        <>
          {server.allow.length === 0 && draft.allow.length === 0 ? (
            <EmptyState
              icon={DownloadIcon}
              title="Choose where to read from"
              description="Ingest only reads project folders you list below. Detect projects from your tools' history or type a path."
              focusAction={false}
              className="border py-10"
            />
          ) : null}
          <div className="grid items-start gap-4 lg:grid-cols-[1fr_22rem]">
            <SourcesEditor
              draft={draft}
              onChange={(next) => {
                setDraft(next)
                setPathErrors({})
              }}
              server={server}
              dirty={dirty}
              saving={save.isPending}
              onSave={onSave}
              onReset={() => {
                setDraft(savedBody)
                setPathErrors({})
              }}
              pathErrors={pathErrors}
            />
            <RunPanel
              runId={runId}
              canRun={server.enabled && !dirty}
              blockedReason={blockedReason}
              starting={run.isPending}
              cancelling={cancel.isPending}
              onRun={onRun}
              onCancel={() =>
                runId &&
                cancel.mutate(runId, {
                  onError: (error) =>
                    toastError(error, "Could not cancel the run"),
                })
              }
              last={lastFinished}
            />
          </div>
          <RunHistory ws={ws} runs={runs.data} />
        </>
      ) : null}
    </section>
  )
}
