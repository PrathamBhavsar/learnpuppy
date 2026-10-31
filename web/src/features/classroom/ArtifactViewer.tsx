import { useArtifact } from "@/api/queries/chats"
import type { ClassroomJob } from "@/api/pending-types"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonRows } from "@/components/app/skeleton-block"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { useEndSide } from "@/hooks/use-end-side"
import { toApiError } from "@/api/errors"

import { LazyMarkdown } from "./markdown/LazyMarkdown"

type ArtifactViewerProps = {
  ws: string
  job: ClassroomJob | null
  onClose: () => void
}

function Body({ ws, job }: { ws: string; job: ClassroomJob }) {
  const artifact = useArtifact(ws, job.artifactId ?? null)
  if (artifact.isPending) return <SkeletonRows rows={6} className="p-4" />
  if (artifact.isError) {
    const error = toApiError(artifact.error)
    return (
      <ErrorState
        title="Could not load the result"
        message={error.message}
        requestId={error.requestId}
        onRetry={() => artifact.refetch()}
      />
    )
  }
  const data = artifact.data
  if (data.kind === "image")
    return (
      <img
        src={data.url}
        alt={job.title}
        data-testid="artifact-image"
        className="max-w-full p-4"
      />
    )
  if (data.contentType.startsWith("text/html"))
    // sandbox="" grants nothing: no scripts, no forms, no same-origin access.
    return (
      <iframe
        title={job.title}
        sandbox=""
        srcDoc={data.text}
        data-testid="artifact-html"
        className="size-full min-h-96 flex-1 border-0 bg-white"
      />
    )
  if (data.contentType.startsWith("text/markdown"))
    return (
      <div className="p-4" data-testid="artifact-markdown">
        <LazyMarkdown>{data.text}</LazyMarkdown>
      </div>
    )
  return (
    <pre
      data-testid="artifact-text"
      className="overflow-auto p-4 text-[12px] whitespace-pre-wrap"
    >
      {data.text}
    </pre>
  )
}

/** Job output in a side sheet: Markdown inline, HTML in an inert iframe, images as-is. */
export function ArtifactViewer({ ws, job, onClose }: ArtifactViewerProps) {
  const side = useEndSide()
  return (
    <Sheet open={job !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent
        side={side}
        className="flex w-full flex-col gap-0 sm:max-w-xl"
        data-testid="artifact-viewer"
      >
        <SheetHeader>
          <SheetTitle>{job?.title ?? "Result"}</SheetTitle>
          <SheetDescription>Output of a background job.</SheetDescription>
        </SheetHeader>
        <div className="flex min-h-0 flex-1 flex-col overflow-auto">
          {job ? <Body ws={ws} job={job} /> : null}
        </div>
      </SheetContent>
    </Sheet>
  )
}
