import { zodResolver } from "@hookform/resolvers/zod"
import { useEffect, useMemo } from "react"
import { useForm, useWatch } from "react-hook-form"

import { toApiError } from "@/api/errors"
import type { WorkspaceListItem } from "@/api/pending-types"
import {
  useCreateWorkspace,
  useUpdateWorkspace,
} from "@/api/queries/workspaces"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { toastError, toastSuccess } from "@/lib/toast"
import { cn } from "@/lib/utils"
import {
  WORKSPACE_ICONS,
  hueClass,
  type WorkspaceIconKey,
} from "@/lib/workspace-visual"

import {
  DESCRIPTION_MAX,
  HUES,
  ICON_KEYS,
  NAME_MAX,
  type WorkspaceFormValues,
  isNameTaken,
  workspaceSchema,
} from "./model"

type FieldProps = {
  id: string
  label: string
  error?: string | undefined
  children: React.ReactNode
}

function Field({ id, label, error, children }: FieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
      {error ? (
        <p
          id={`${id}-error`}
          role="alert"
          data-testid={`${id}-error`}
          className="text-[12px] text-destructive"
        >
          {error}
        </p>
      ) : null}
    </div>
  )
}

type PickerProps<K extends string> = {
  name: string
  label: string
  value: K
  options: readonly K[]
  onChange: (value: K) => void
  render: (key: K) => React.ReactNode
  className?: string
  itemClassName: (key: K, selected: boolean) => string
}

/** Radio-group-as-grid: native radios give arrow-key navigation that follows reading direction. */
function Picker<K extends string>({
  name,
  label,
  value,
  options,
  onChange,
  render,
  className,
  itemClassName,
}: PickerProps<K>) {
  return (
    <fieldset className="flex flex-col gap-1.5">
      <legend className="mb-1.5 text-[12px] font-medium">{label}</legend>
      <div role="radiogroup" aria-label={label} className={className}>
        {options.map((key) => (
          <label
            key={key}
            title={key}
            data-testid={`${name}-${key}`}
            className={cn(
              "relative grid cursor-pointer place-items-center has-[:focus-visible]:ring-1 has-[:focus-visible]:ring-ring",
              itemClassName(key, key === value)
            )}
          >
            <input
              type="radio"
              name={name}
              value={key}
              checked={key === value}
              onChange={() => onChange(key)}
              aria-label={key}
              className="peer sr-only"
            />
            {render(key)}
          </label>
        ))}
      </div>
    </fieldset>
  )
}

type FormProps = {
  workspace?: WorkspaceListItem | undefined
  existing: readonly WorkspaceListItem[]
  onDone: (created?: WorkspaceListItem) => void
  /** Shows a Cancel button; omitted on the onboarding page. */
  cancellable?: boolean
  submitLabel?: string
  focusName?: boolean
}

/** Name, icon, color and description: the body of both the dialog and the onboarding page. */
export function WorkspaceForm({
  workspace,
  existing,
  onDone,
  cancellable = true,
  submitLabel,
  focusName = true,
}: FormProps) {
  const create = useCreateWorkspace()
  const update = useUpdateWorkspace(workspace?.id ?? "")
  const resolver = useMemo(
    () =>
      zodResolver(
        workspaceSchema((name) => isNameTaken(name, existing, workspace?.id))
      ),
    [existing, workspace?.id]
  )
  const form = useForm<WorkspaceFormValues>({
    resolver,
    mode: "onChange",
    defaultValues: {
      name: workspace?.name ?? "",
      icon: (workspace?.icon as WorkspaceIconKey | undefined) ?? "book-open",
      hue: (HUES.find((h) => h === workspace?.hue) ??
        "violet") as (typeof HUES)[number],
      description: workspace?.description ?? "",
    },
  })
  const {
    register,
    handleSubmit,
    setError,
    setFocus,
    control,
    setValue,
    trigger,
    formState: { errors, isSubmitting, isValid },
  } = form

  useEffect(() => {
    if (focusName) setFocus("name")
  }, [setFocus, focusName])
  // The list can change while the dialog is open (another tab created the same name).
  useEffect(() => {
    if (form.formState.dirtyFields.name) void trigger("name")
  }, [existing, trigger, form.formState.dirtyFields.name])

  const icon = useWatch({ control, name: "icon" })
  const hue = useWatch({ control, name: "hue" })
  const description = useWatch({ control, name: "description" })
  const editing = Boolean(workspace)

  const onSubmit = handleSubmit(async (values) => {
    const body = { ...values, name: values.name.trim() }
    try {
      if (workspace) {
        await update.mutateAsync(body)
        toastSuccess("Workspace updated")
        onDone()
      } else {
        const created = await create.mutateAsync(body)
        toastSuccess(`Created ${created.name}`)
        onDone(created)
      }
    } catch (error) {
      const api = toApiError(error)
      const known = api.fields.filter((f) => f.path in values)
      if (known.length > 0) {
        for (const f of known)
          setError(f.path as keyof WorkspaceFormValues, { message: f.message })
      } else {
        toastError(api, editing ? "Could not update" : "Could not create")
      }
    }
  })

  return (
    <form
      noValidate
      onSubmit={(event) => {
        event.preventDefault()
        void onSubmit(event)
      }}
      className="flex flex-col gap-4"
      data-testid="workspace-form"
    >
      <Field id="ws-name" label="Name" error={errors.name?.message}>
        <Input
          id="ws-name"
          data-testid="ws-name-input"
          autoComplete="off"
          maxLength={NAME_MAX + 20}
          aria-invalid={errors.name ? true : undefined}
          aria-describedby={errors.name ? "ws-name-error" : undefined}
          {...register("name")}
        />
      </Field>
      <Picker
        name="ws-icon"
        label="Icon"
        value={icon}
        options={ICON_KEYS}
        onChange={(v) => setValue("icon", v, { shouldDirty: true })}
        className="grid grid-cols-8 gap-1"
        itemClassName={(_k, selected) =>
          cn(
            "size-8 border text-muted-foreground transition-colors duration-[var(--dur-instant)] hover:bg-muted",
            selected && "border-ring bg-muted text-foreground"
          )
        }
        render={(key) => {
          const Icon = WORKSPACE_ICONS[key]
          return <Icon aria-hidden className="size-4" />
        }}
      />
      <Picker
        name="ws-hue"
        label="Color"
        value={hue}
        options={HUES}
        onChange={(v) => setValue("hue", v, { shouldDirty: true })}
        className="flex gap-2"
        itemClassName={(k, selected) =>
          cn(
            "size-7 border-2 border-transparent",
            hueClass(k),
            selected && "border-foreground"
          )
        }
        render={() => null}
      />
      <Field
        id="ws-description"
        label="Description"
        error={errors.description?.message}
      >
        <Textarea
          id="ws-description"
          data-testid="ws-description-input"
          rows={3}
          aria-invalid={errors.description ? true : undefined}
          {...register("description")}
        />
        <span
          aria-hidden
          className="self-end text-[11px] text-muted-foreground tabular-nums"
        >
          {description.length}/{DESCRIPTION_MAX}
        </span>
      </Field>
      <DialogFooter>
        {cancellable ? (
          <Button
            type="button"
            variant="ghost"
            onClick={() => onDone()}
            data-testid="ws-cancel"
          >
            Cancel
          </Button>
        ) : null}
        <Button
          type="submit"
          disabled={isSubmitting || !isValid}
          data-testid="ws-submit"
        >
          {isSubmitting ? <Spinner data-icon="inline-start" /> : null}
          {submitLabel ?? (editing ? "Save changes" : "Create workspace")}
        </Button>
      </DialogFooter>
    </form>
  )
}

type WorkspaceDialogProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Present = edit mode. */
  workspace?: WorkspaceListItem | undefined
  /** Live workspaces, for the uniqueness check. */
  existing: readonly WorkspaceListItem[]
  onCreated?: (workspace: WorkspaceListItem) => void
}

export function WorkspaceDialog({
  open,
  onOpenChange,
  workspace,
  existing,
  onCreated,
}: WorkspaceDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent data-testid="workspace-dialog">
        <DialogHeader>
          <DialogTitle>
            {workspace ? "Edit workspace" : "New workspace"}
          </DialogTitle>
          <DialogDescription>
            {workspace
              ? "Change how this workspace looks and what it is about."
              : "A workspace is its own knowledge graph, chats and review cards."}
          </DialogDescription>
        </DialogHeader>
        <WorkspaceForm
          workspace={workspace}
          existing={existing}
          onDone={(created) => {
            onOpenChange(false)
            if (created) onCreated?.(created)
          }}
        />
      </DialogContent>
    </Dialog>
  )
}
