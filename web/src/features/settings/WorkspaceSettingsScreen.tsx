import { ScreenGuide } from "@/components/app/info/info"
import { Link } from "@tanstack/react-router"
import { ArchiveIcon, Trash2Icon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { WorkspaceDetail } from "@/api/pending-types"
import { useSettings, useSettingsCatalog } from "@/api/queries/settings"
import {
  useDeleteWorkspace,
  useSetWorkspaceArchived,
  useUpdateWorkspace,
  useWorkspace,
} from "@/api/queries/workspaces"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { RoleSlots } from "@/components/app/role-slots/RoleSlots"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { useGoto } from "@/hooks/use-goto"
import { usePageTitle } from "@/hooks/use-page-title"
import { toastError, toastSuccess } from "@/lib/toast"
import {
  WORKSPACE_ICONS,
  hueClass,
  workspaceIcon,
} from "@/lib/workspace-visual"
import { cn } from "@/lib/utils"

import { WS_SETTINGS_GUIDE_PARTS, WS_SETTINGS_INFO } from "./help"
import { MethodSettingsSection } from "./MethodSettingsSection"
import { Section } from "./Section"
import { SettingRow } from "./SettingRow"

const HUES = [
  "violet",
  "teal",
  "orange",
  "blue",
  "sprout",
  "rose",
  "gold",
  "lilac",
] as const
const ICON_KEYS = Object.keys(WORKSPACE_ICONS)

function GeneralForm({ detail }: { detail: WorkspaceDetail }) {
  const update = useUpdateWorkspace(detail.id)
  const [name, setName] = useState(detail.name)
  const [description, setDescription] = useState(detail.description)
  const [icon, setIcon] = useState(detail.icon)
  const [hue, setHue] = useState(detail.hue)
  const [nameError, setNameError] = useState<string | null>(null)

  const dirty =
    name !== detail.name ||
    description !== detail.description ||
    icon !== detail.icon ||
    hue !== detail.hue

  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    if (name.trim() === "") {
      setNameError("Give the workspace a name")
      return
    }
    setNameError(null)
    update.mutate(
      { name: name.trim(), description, icon, hue },
      {
        onSuccess: () => toastSuccess("Workspace saved"),
        onError: (error) => {
          const field = toApiError(error).fields.find((f) => f.path === "name")
          if (field) setNameError(field.message)
          else toastError(error, "Could not save the workspace")
        },
      }
    )
  }

  return (
    <form
      onSubmit={submit}
      noValidate
      data-testid="ws-general"
      className="flex flex-col gap-4"
    >
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="ws-name">Name</Label>
        <Input
          id="ws-name"
          data-testid="ws-name"
          value={name}
          aria-invalid={nameError ? true : undefined}
          aria-describedby={nameError ? "ws-name-error" : undefined}
          onChange={(e) => {
            setName(e.target.value)
            setNameError(null)
          }}
        />
        {nameError ? (
          <p
            id="ws-name-error"
            role="alert"
            className="text-[12px] text-destructive"
            data-testid="ws-name-error"
          >
            {nameError}
          </p>
        ) : null}
      </div>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="ws-description">Description</Label>
        <Textarea
          id="ws-description"
          data-testid="ws-description"
          rows={3}
          value={description}
          onChange={(e) => setDescription(e.target.value)}
        />
      </div>
      <div className="grid gap-4 sm:grid-cols-2">
        <fieldset className="flex flex-col gap-1.5">
          <legend className="mb-1 text-[12px] font-medium">Icon</legend>
          <div
            role="radiogroup"
            aria-label="Icon"
            className="grid grid-cols-8 gap-1"
          >
            {ICON_KEYS.map((key) => {
              const Icon = workspaceIcon(key)
              return (
                <label
                  key={key}
                  title={key}
                  className={cn(
                    "relative grid size-8 cursor-pointer place-items-center border has-[:focus-visible]:ring-1 has-[:focus-visible]:ring-ring",
                    key === icon && "border-primary bg-accent"
                  )}
                >
                  <input
                    type="radio"
                    name="ws-icon"
                    className="peer sr-only"
                    checked={key === icon}
                    aria-label={key}
                    onChange={() => setIcon(key)}
                  />
                  <Icon className="size-4" aria-hidden />
                </label>
              )
            })}
          </div>
        </fieldset>
        <fieldset className="flex flex-col gap-1.5">
          <legend className="mb-1 text-[12px] font-medium">Color</legend>
          <div
            role="radiogroup"
            aria-label="Color"
            className="flex flex-wrap gap-1.5"
          >
            {HUES.map((key) => (
              <label
                key={key}
                title={key}
                className={cn(
                  "relative grid size-8 cursor-pointer place-items-center border has-[:focus-visible]:ring-1 has-[:focus-visible]:ring-ring",
                  key === hue && "border-primary"
                )}
              >
                <input
                  type="radio"
                  name="ws-hue"
                  className="peer sr-only"
                  checked={key === hue}
                  aria-label={key}
                  onChange={() => setHue(key)}
                />
                <span className={cn("size-5", hueClass(key))} aria-hidden />
              </label>
            ))}
          </div>
        </fieldset>
      </div>
      <div className="flex justify-end">
        <Button
          type="submit"
          size="sm"
          disabled={!dirty || update.isPending}
          data-testid="ws-save"
        >
          {update.isPending ? <Spinner data-icon="inline-start" /> : null}
          Save changes
        </Button>
      </div>
    </form>
  )
}

function OverridesSection({ detail }: { detail: WorkspaceDetail }) {
  const settings = useSettings()
  const catalog = useSettingsCatalog()
  const update = useUpdateWorkspace(detail.id)

  if (settings.isError || catalog.isError) {
    const error = toApiError(settings.error ?? catalog.error)
    return (
      <ErrorState
        title="Model settings did not load"
        message={error.message}
        requestId={error.requestId}
        onRetry={() => Promise.all([settings.refetch(), catalog.refetch()])}
        className="border py-8"
      />
    )
  }
  if (!settings.data || !catalog.data)
    return (
      <SkeletonBlock className="h-64 w-full" data-testid="overrides-loading" />
    )

  return (
    <RoleSlots
      testIdPrefix="ws"
      catalog={catalog.data}
      saved={detail.overrides.profiles}
      inherited={settings.data.profiles}
      save={(profiles) => update.mutateAsync({ overrides: { profiles } })}
    />
  )
}

/** Archived workspaces drop out of `/w/:ws/*`, so this only archives; restoring lives on the home screen. */
function ArchiveRow({ detail }: { detail: WorkspaceDetail }) {
  const archive = useSetWorkspaceArchived()
  const goto = useGoto()
  return (
    <SettingRow
      label="Archive workspace"
      description="Make it read-only and hide it from the sidebar. Nothing is deleted, and you can restore it from the home screen."
    >
      <Button
        variant="outline"
        size="sm"
        disabled={archive.isPending}
        data-testid="ws-archive"
        onClick={() =>
          archive.mutate(
            { ws: detail.id, archived: true },
            {
              onSuccess: () => {
                toastSuccess("Workspace archived")
                goto("/")
              },
              onError: (error) =>
                toastError(error, "Could not archive the workspace"),
            }
          )
        }
      >
        {archive.isPending ? (
          <Spinner data-icon="inline-start" />
        ) : (
          <ArchiveIcon data-icon="inline-start" />
        )}
        Archive
      </Button>
    </SettingRow>
  )
}

function DeleteRow({ detail }: { detail: WorkspaceDetail }) {
  const [open, setOpen] = useState(false)
  const [typed, setTyped] = useState("")
  const remove = useDeleteWorkspace()
  const goto = useGoto()

  const close = (next: boolean) => {
    setOpen(next)
    if (!next) setTyped("")
  }

  return (
    <>
      <SettingRow
        label="Delete workspace"
        description="Moves it to the trash, where it can be restored or purged."
      >
        <Button
          variant="destructive"
          size="sm"
          data-testid="ws-delete"
          onClick={() => setOpen(true)}
        >
          <Trash2Icon data-icon="inline-start" />
          Delete
        </Button>
      </SettingRow>
      <AlertDialog open={open} onOpenChange={close}>
        <AlertDialogContent data-testid="ws-delete-dialog">
          <AlertDialogHeader>
            <AlertDialogTitle>Delete {detail.name}?</AlertDialogTitle>
            <AlertDialogDescription>
              It moves to the trash. Type the workspace name to confirm.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <div className="flex flex-col gap-1.5">
            <Label htmlFor="ws-delete-confirm">Workspace name</Label>
            <Input
              id="ws-delete-confirm"
              data-testid="ws-delete-confirm"
              autoComplete="off"
              value={typed}
              onChange={(e) => setTyped(e.target.value)}
            />
          </div>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <Button
              variant="destructive"
              disabled={typed !== detail.name || remove.isPending}
              data-testid="ws-delete-submit"
              onClick={() =>
                remove.mutate(
                  { ws: detail.id, confirmName: typed },
                  {
                    onSuccess: () => {
                      toastSuccess("Workspace moved to the trash")
                      goto("/")
                    },
                    onError: (error) =>
                      toastError(error, "Could not delete the workspace"),
                  }
                )
              }
            >
              {remove.isPending ? <Spinner data-icon="inline-start" /> : null}
              Delete workspace
            </Button>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}

export function WorkspaceSettingsScreen({ ws }: { ws: string }) {
  const workspace = useWorkspace(ws)
  const detail = workspace.data
  usePageTitle("Workspace settings", detail?.name)

  const tools = detail
    ? (["claude", "codex", "agy", "git"] as const).filter(
        (t) => detail.sources[t]
      )
    : []

  return (
    <section
      data-testid="screen-settings"
      aria-busy={workspace.isPending}
      className="mx-auto flex min-h-full w-full max-w-3xl flex-col gap-6 p-6"
    >
      <PageHeader
        title="Workspace settings"
        description="Name, sources and model overrides for this workspace."
        actions={
          <ScreenGuide
            title="Workspace settings"
            summary="Settings for this one workspace only."
            parts={WS_SETTINGS_GUIDE_PARTS}
          />
        }
      />
      {workspace.isPending ? (
        <div data-testid="settings-loading" className="flex flex-col gap-4">
          {[0, 1, 2].map((i) => (
            <SkeletonBlock key={i} className="h-40 w-full" />
          ))}
        </div>
      ) : workspace.isError || !detail ? (
        <ErrorState
          title="Workspace did not load"
          message={toApiError(workspace.error).message}
          requestId={toApiError(workspace.error).requestId}
          onRetry={() => workspace.refetch()}
          className="border"
        />
      ) : (
        <>
          <Section id="general" title="General" info={WS_SETTINGS_INFO.general}>
            <GeneralForm
              key={`${detail.name}|${detail.description}|${detail.icon}|${detail.hue}`}
              detail={detail}
            />
          </Section>
          <Section id="sources" title="Sources" info={WS_SETTINGS_INFO.sources}>
            <SettingRow
              label={`${detail.sources.allow.length} ${detail.sources.allow.length === 1 ? "folder" : "folders"}`}
              description={
                tools.length > 0
                  ? `Reading ${tools.join(", ")}.`
                  : "No tools selected."
              }
            >
              <Button asChild variant="outline" size="sm">
                <Link
                  to="/w/$ws/ingest"
                  params={{ ws }}
                  data-testid="ws-sources-link"
                >
                  Edit sources
                </Link>
              </Button>
            </SettingRow>
          </Section>
          <Section
            id="overrides"
            title="Model overrides"
            info={WS_SETTINGS_INFO.overrides}
            description="Override a role for this workspace only. Roles left off inherit the global setting from Accounts."
          >
            <OverridesSection detail={detail} />
          </Section>
          <Section
            id="method"
            title="Study method"
            info={WS_SETTINGS_INFO.method}
            description="How long a bout runs, how much rest it needs and how strict the teach loop is. These are starting values, not facts: change any of them."
          >
            <MethodSettingsSection ws={detail.id} />
          </Section>
          <Section
            id="danger"
            title="Archive and delete"
            tone="danger"
            info={WS_SETTINGS_INFO.danger}
          >
            <div className="flex flex-col divide-y">
              <ArchiveRow detail={detail} />
              <DeleteRow detail={detail} />
            </div>
          </Section>
        </>
      )}
    </section>
  )
}
