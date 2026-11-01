import { Link, useNavigate } from "@tanstack/react-router"
import {
  LayersIcon,
  PlusIcon,
  TriangleAlertIcon,
  UploadIcon,
  FolderPlusIcon,
  Trash2Icon,
} from "lucide-react"
import { useEffect, useState } from "react"

import { toApiError } from "@/api/errors"
import type { WorkspaceListItem } from "@/api/pending-types"
import { useAccounts } from "@/api/queries/accounts"
import {
  useSetWorkspaceArchived,
  useWorkspaces,
} from "@/api/queries/workspaces"
import { HelpMenu } from "@/components/app/help-menu"
import { InfoTip, ScreenGuide } from "@/components/app/info/info"
import { DirectionToggle } from "@/components/app/direction-toggle"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { ThemeToggle } from "@/components/app/theme-toggle"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { usePageTitle } from "@/hooks/use-page-title"
import { toastError, toastInfo, toastSuccess } from "@/lib/toast"
import { useStatus } from "@/stores/status"

import { useDataDir, useRepairWorkspace } from "./api"
import { DeleteWorkspaceDialog } from "./DeleteWorkspaceDialog"
import { useStartExport } from "./exports"
import { ImportDialog } from "./ImportDialog"
import { Onboarding } from "./Onboarding"
import type { CardActions } from "./WorkspaceCard"
import { HOME_GUIDE_PARTS, HOME_INFO } from "./help"
import { WorkspaceDialog } from "./WorkspaceDialog"
import { WorkspaceGrid, WorkspaceGridSkeleton } from "./WorkspaceGrid"

type Props = {
  /** `/?new=1`: open the create dialog on arrival (sidebar chip and palette link here). */
  newRequested: boolean
}

export function HomeScreen({ newRequested }: Props) {
  usePageTitle("Workspaces")
  const navigate = useNavigate()
  const live = useWorkspaces(false)
  const archivedQuery = useWorkspaces(true)
  const accounts = useAccounts()
  const engineUnavailable = useStatus((s) => s.engineUnavailable)
  const dataDir = useDataDir()
  const setArchived = useSetWorkspaceArchived()
  const repair = useRepairWorkspace()
  const exporter = useStartExport()

  const [showArchived, setShowArchived] = useState(false)
  const [creating, setCreating] = useState(newRequested)
  const [importing, setImporting] = useState(false)
  const [editing, setEditing] = useState<WorkspaceListItem | null>(null)
  const [deleting, setDeleting] = useState<WorkspaceListItem | null>(null)
  const [freshId, setFreshId] = useState<string | null>(null)
  const [onboarding, setOnboarding] = useState(false)

  const liveList = live.data ?? []
  const archivedList = archivedQuery.data ?? []
  const firstRun =
    live.isSuccess &&
    archivedQuery.isSuccess &&
    liveList.length === 0 &&
    archivedList.length === 0

  // Latch onboarding so creating the first workspace does not swap the page out from under step 3.
  if (firstRun && !onboarding) setOnboarding(true)

  useEffect(() => {
    if (!freshId) return
    const timer = window.setTimeout(() => setFreshId(null), 1500)
    return () => window.clearTimeout(timer)
  }, [freshId])

  const closeCreate = (open: boolean) => {
    setCreating(open)
    if (!open && newRequested)
      void navigate({ to: "/", search: {}, replace: true })
  }

  const actions: CardActions = {
    onEdit: setEditing,
    onDelete: setDeleting,
    onExport: (w) => exporter.start(w),
    onSetArchived: (w, archived) =>
      setArchived.mutate(
        { ws: w.id, archived },
        {
          onSuccess: () =>
            toastSuccess(
              archived ? `Archived ${w.name}` : `Restored ${w.name}`
            ),
          onError: (error) =>
            toastError(
              error,
              archived ? "Could not archive" : "Could not restore"
            ),
        }
      ),
    onRepair: (w) =>
      repair.mutate(w.id, {
        onSuccess: () => toastSuccess(`Repaired ${w.name}`),
        onError: (error) => toastError(error, "Could not repair"),
      }),
    onReveal: (w) => {
      const path = `${dataDir.data ?? "~/.learnpuppy"}/workspaces/${w.id}`
      void navigator.clipboard?.writeText(path)
      toastInfo("Path copied", path)
    },
  }

  const due = liveList.reduce((sum, w) => sum + w.stats.dueCards, 0)
  const dueIn = liveList.filter((w) => w.stats.dueCards > 0).length
  const damaged = [...liveList, ...archivedList].filter((w) => w.damaged).length
  const noAccount =
    accounts.isSuccess && !accounts.data.some((a) => a.status === "ready")
  const degraded = damaged > 0 || noAccount || engineUnavailable
  const error = live.isError ? toApiError(live.error) : null
  const showOnboarding = onboarding || firstRun

  const header = (
    <PageHeader
      title="Workspaces"
      description="Each workspace is its own knowledge base."
      actions={
        <>
          <ScreenGuide
            title="Workspaces"
            summary="The home screen lists every workspace, one per subject. Here is what each part does."
            parts={HOME_GUIDE_PARTS}
          />
          {showOnboarding ? null : (
            <>
              {due > 0 ? (
                <Button asChild variant="secondary" size="sm">
                  <Link to="/review-all" data-testid="due-all-link">
                    <LayersIcon data-icon="inline-start" />
                    <span className="tabular-nums">
                      Due today: {due} across {dueIn}
                    </span>
                  </Link>
                </Button>
              ) : null}
              {archivedList.length > 0 ? (
                <div className="flex items-center gap-1.5">
                  <Switch
                    id="show-archived"
                    checked={showArchived}
                    onCheckedChange={setShowArchived}
                    data-testid="archived-toggle"
                  />
                  <Label htmlFor="show-archived" className="text-[12px]">
                    Archived
                    <Badge variant="outline" className="tabular-nums">
                      {archivedList.length}
                    </Badge>
                  </Label>
                  <InfoTip info={HOME_INFO.archived} />
                </div>
              ) : null}
              <Button variant="ghost" size="sm" asChild>
                <Link to="/trash" data-testid="trash-link">
                  <Trash2Icon data-icon="inline-start" />
                  Trash
                </Link>
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setImporting(true)}
                data-testid="import-open"
              >
                <UploadIcon data-icon="inline-start" />
                Import
              </Button>
              <Button
                size="sm"
                onClick={() => setCreating(true)}
                data-testid="new-workspace"
                data-tour="home-new"
              >
                <PlusIcon data-icon="inline-start" />
                New workspace
              </Button>
            </>
          )}
        </>
      }
    />
  )

  return (
    <div
      className="lp-pane h-dvh w-dvw overflow-y-auto bg-background text-foreground"
      data-testid="home"
      aria-busy={live.isPending}
    >
      <div className="mx-auto flex max-w-5xl flex-col gap-6 p-8">
        <div className="flex items-start justify-between gap-4">
          <div className="min-w-0 flex-1" data-tour="home-header">
            {header}
          </div>
          <div className="flex items-center gap-1">
            <div className="flex items-center gap-1" data-tour="appearance">
              <DirectionToggle />
              <ThemeToggle />
            </div>
            <HelpMenu />
          </div>
        </div>

        {degraded && !showOnboarding ? (
          <Alert data-testid="home-degraded">
            <TriangleAlertIcon />
            <AlertTitle>Some things need attention</AlertTitle>
            <AlertDescription>
              {damaged > 0
                ? `${damaged} workspace${damaged === 1 ? " is" : "s are"} damaged. Repair rewrites its metadata. `
                : ""}
              {noAccount || engineUnavailable
                ? "No model account is ready, so chats and ingest cannot run. "
                : ""}
              {noAccount || engineUnavailable ? (
                <Link to="/accounts" className="underline">
                  Open accounts
                </Link>
              ) : null}
            </AlertDescription>
          </Alert>
        ) : null}

        {live.isPending ? (
          <WorkspaceGridSkeleton testId="home-loading" />
        ) : error ? (
          <ErrorState
            title="Could not load workspaces"
            message={error.message}
            {...(error.requestId ? { requestId: error.requestId } : {})}
            onRetry={() => live.refetch()}
          />
        ) : showOnboarding ? (
          <Onboarding onImport={() => setImporting(true)} />
        ) : (
          <>
            {liveList.length === 0 ? (
              <EmptyState
                icon={FolderPlusIcon}
                title="No active workspaces"
                description="Everything is archived. Restore one below or start a new workspace."
                action={
                  <Button
                    onClick={() => setCreating(true)}
                    data-testid="empty-new-workspace"
                  >
                    <PlusIcon data-icon="inline-start" />
                    New workspace
                  </Button>
                }
              />
            ) : (
              <WorkspaceGrid
                label="Workspaces"
                workspaces={liveList}
                actions={actions}
                freshId={freshId}
                busyId={repair.isPending ? repair.variables : null}
              />
            )}
            {showArchived && archivedList.length > 0 ? (
              <section
                aria-labelledby="archived-heading"
                className="flex flex-col gap-3"
              >
                <h2
                  id="archived-heading"
                  className="font-heading text-[14px] font-medium text-muted-foreground"
                >
                  Archived
                </h2>
                <WorkspaceGrid
                  label="Archived workspaces"
                  testId="archived-grid"
                  workspaces={archivedList}
                  actions={actions}
                />
              </section>
            ) : null}
          </>
        )}
      </div>

      <WorkspaceDialog
        open={creating}
        onOpenChange={closeCreate}
        existing={liveList}
        onCreated={(w) => {
          setFreshId(w.id)
        }}
      />
      <WorkspaceDialog
        open={editing !== null}
        onOpenChange={(open) => {
          if (!open) setEditing(null)
        }}
        workspace={editing ?? undefined}
        existing={liveList}
      />
      <DeleteWorkspaceDialog
        workspace={deleting}
        onOpenChange={(open) => {
          if (!open) setDeleting(null)
        }}
      />
      <ImportDialog
        open={importing}
        onOpenChange={setImporting}
        onImported={(id) => setFreshId(id)}
      />
    </div>
  )
}
