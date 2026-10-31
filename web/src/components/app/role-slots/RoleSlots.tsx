import { FlaskConicalIcon, SaveIcon } from "lucide-react"
import { useMemo, useState } from "react"

import { type FieldError, toApiError } from "@/api/errors"
import type {
  ProfilePingResult,
  RoleProfile,
  SettingsCatalog,
} from "@/api/pending-types"
import { usePingProfile } from "@/api/queries/settings"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { toastError, toastSuccess } from "@/lib/toast"

import {
  type ProfileChange,
  diffProfiles,
  providerLabel,
  roleLabel,
  without,
  withProvider,
} from "./model"

type RoleSlotsProps = {
  catalog: SettingsCatalog
  /** Persisted profiles. Global mode: every role. Override mode: only the overridden roles. */
  saved: Record<string, RoleProfile>
  /** Override mode: the global profiles that unoverridden roles inherit. */
  inherited?: Record<string, RoleProfile>
  save: (profiles: Record<string, RoleProfile>) => Promise<unknown>
  testIdPrefix: string
}

type PingState =
  | { status: "pending" }
  | { status: "done"; result: ProfilePingResult }
  | { status: "unavailable" }

const DEFAULT_EFFORT = "default"

function fieldFor(errors: FieldError[], role: string, field: string) {
  return errors.find((e) => e.path === `profiles.${role}.${field}`)?.message
}

/**
 * Role -> provider/model/effort/fallback table shared by Accounts (global) and workspace settings
 * (per-role overrides). Edits stay in a draft until the diff preview is confirmed.
 */
export function RoleSlots({
  catalog,
  saved,
  inherited,
  save,
  testIdPrefix,
}: RoleSlotsProps) {
  const overrideMode = inherited !== undefined
  const [draft, setDraft] = useState<Record<string, RoleProfile>>(saved)
  const [savedSnapshot, setSavedSnapshot] = useState(saved)
  const [reviewing, setReviewing] = useState(false)
  const [saving, setSaving] = useState(false)
  const [errors, setErrors] = useState<FieldError[]>([])
  const [pings, setPings] = useState<Record<string, PingState>>({})
  const ping = usePingProfile()

  // Follow the server copy after a save or refetch, unless the user is mid-edit.
  if (savedSnapshot !== saved) {
    setSavedSnapshot(saved)
    if (diffProfiles(savedSnapshot, draft).length === 0) setDraft(saved)
  }

  const changes = useMemo(() => diffProfiles(saved, draft), [saved, draft])
  const dirty = changes.length > 0

  const effective = (role: string): RoleProfile | undefined =>
    draft[role] ?? inherited?.[role]

  const update = (role: string, next: RoleProfile) => {
    setDraft((d) => ({ ...d, [role]: next }))
    setPings((p) => without(p, role))
  }

  const modelsFor = (provider: string) =>
    catalog.providers.find((p) => p.id === provider)?.models ?? []

  const runPing = async (role: string) => {
    const profile = effective(role)
    if (!profile) return
    setPings((p) => ({ ...p, [role]: { status: "pending" } }))
    try {
      const result = await ping.mutateAsync({
        provider: profile.provider,
        model: profile.model,
        effort: profile.effort ?? null,
      })
      setPings((p) => ({ ...p, [role]: { status: "done", result } }))
    } catch (error) {
      const api = toApiError(error)
      if (api.status === 404 || api.status === 405) {
        setPings((p) => ({ ...p, [role]: { status: "unavailable" } }))
      } else {
        setPings((p) => without(p, role))
        toastError(error, "Test ping failed")
      }
    }
  }

  const confirm = async () => {
    setSaving(true)
    setErrors([])
    try {
      await save(draft)
      setReviewing(false)
      toastSuccess("Role slots saved")
    } catch (error) {
      const api = toApiError(error)
      setReviewing(false)
      if (api.fields.length > 0) setErrors(api.fields)
      else toastError(error, "Could not save role slots")
    } finally {
      setSaving(false)
    }
  }

  return (
    <div
      data-testid={`${testIdPrefix}-role-slots`}
      className="flex flex-col gap-3"
    >
      <div className="overflow-x-auto border">
        <table className="w-full min-w-[44rem] text-[13px]">
          <thead>
            <tr className="border-b bg-muted/40 text-start text-[11px] text-muted-foreground">
              <th scope="col" className="px-3 py-2 text-start font-medium">
                Role
              </th>
              {overrideMode ? (
                <th scope="col" className="px-3 py-2 text-start font-medium">
                  Override
                </th>
              ) : null}
              <th scope="col" className="px-3 py-2 text-start font-medium">
                Provider
              </th>
              <th scope="col" className="px-3 py-2 text-start font-medium">
                Model
              </th>
              <th scope="col" className="px-3 py-2 text-start font-medium">
                Effort
              </th>
              <th scope="col" className="px-3 py-2 text-start font-medium">
                Fallback
              </th>
              <th scope="col" className="px-3 py-2 text-start font-medium">
                <span className="sr-only">Test</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {catalog.roles.map((role) => {
              const profile = effective(role)
              const overriding = role in draft
              const locked = overrideMode && !overriding
              const models = profile ? modelsFor(profile.provider) : []
              const modelError = fieldFor(errors, role, "model")
              const providerError = fieldFor(errors, role, "provider")
              const effortError = fieldFor(errors, role, "effort")
              const state = pings[role]
              return (
                <tr
                  key={role}
                  data-testid={`${testIdPrefix}-role-${role}`}
                  data-locked={locked}
                  className="border-b align-top last:border-b-0"
                >
                  <th
                    scope="row"
                    className="px-3 py-2.5 text-start font-medium"
                  >
                    {roleLabel(role)}
                  </th>
                  {overrideMode ? (
                    <td className="px-3 py-2.5">
                      <Switch
                        size="sm"
                        checked={overriding}
                        aria-label={`Override ${role} in this workspace`}
                        data-testid={`${testIdPrefix}-override-${role}`}
                        onCheckedChange={(on) => {
                          if (on) {
                            const base = inherited?.[role]
                            if (base) update(role, { ...base })
                          } else {
                            setDraft((d) => without(d, role))
                          }
                        }}
                      />
                    </td>
                  ) : null}
                  <td className="px-3 py-2.5">
                    <Select
                      value={profile?.provider ?? ""}
                      disabled={locked || !profile}
                      onValueChange={(provider) =>
                        profile &&
                        update(
                          role,
                          withProvider(profile, provider, modelsFor(provider))
                        )
                      }
                    >
                      <SelectTrigger
                        size="sm"
                        aria-label={`${roleLabel(role)} provider`}
                        aria-invalid={providerError ? true : undefined}
                        data-testid={`${testIdPrefix}-provider-${role}`}
                        className="w-32"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {catalog.providers.map((p) => (
                          <SelectItem key={p.id} value={p.id}>
                            {providerLabel(p.id)}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    {providerError ? (
                      <p
                        role="alert"
                        className="mt-1 text-[11px] text-destructive"
                      >
                        {providerError}
                      </p>
                    ) : null}
                  </td>
                  <td className="px-3 py-2.5">
                    <Select
                      value={profile?.model ?? ""}
                      disabled={locked || !profile}
                      onValueChange={(model) =>
                        profile && update(role, { ...profile, model })
                      }
                    >
                      <SelectTrigger
                        size="sm"
                        aria-label={`${roleLabel(role)} model`}
                        aria-invalid={modelError ? true : undefined}
                        data-testid={`${testIdPrefix}-model-${role}`}
                        className="w-48"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        {models.map((m) => (
                          <SelectItem key={m} value={m}>
                            {m}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                    {modelError ? (
                      <p
                        role="alert"
                        data-testid={`${testIdPrefix}-model-error-${role}`}
                        className="mt-1 max-w-48 text-[11px] text-destructive"
                      >
                        {modelError}
                      </p>
                    ) : null}
                  </td>
                  <td className="px-3 py-2.5">
                    <Select
                      value={profile?.effort ?? DEFAULT_EFFORT}
                      disabled={locked || !profile}
                      onValueChange={(effort) =>
                        profile &&
                        update(role, {
                          ...profile,
                          effort: effort === DEFAULT_EFFORT ? null : effort,
                        })
                      }
                    >
                      <SelectTrigger
                        size="sm"
                        aria-label={`${roleLabel(role)} effort`}
                        aria-invalid={effortError ? true : undefined}
                        data-testid={`${testIdPrefix}-effort-${role}`}
                        className="w-28"
                      >
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value={DEFAULT_EFFORT}>Default</SelectItem>
                        {catalog.efforts.map((e) => (
                          <SelectItem key={e} value={e}>
                            {e}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </td>
                  <td className="px-3 py-2.5">
                    <Switch
                      size="sm"
                      checked={profile?.fallback ?? false}
                      disabled={locked || !profile}
                      aria-label={`${roleLabel(role)} falls back to other providers`}
                      data-testid={`${testIdPrefix}-fallback-${role}`}
                      onCheckedChange={(fallback) =>
                        profile && update(role, { ...profile, fallback })
                      }
                    />
                  </td>
                  <td className="px-3 py-2.5">
                    <div className="flex flex-col items-start gap-1">
                      <Button
                        variant="outline"
                        size="sm"
                        disabled={!profile || state?.status === "pending"}
                        data-testid={`${testIdPrefix}-test-${role}`}
                        onClick={() => void runPing(role)}
                      >
                        {state?.status === "pending" ? (
                          <Spinner data-icon="inline-start" />
                        ) : (
                          <FlaskConicalIcon data-icon="inline-start" />
                        )}
                        Test
                      </Button>
                      <PingResult
                        state={state}
                        role={role}
                        prefix={testIdPrefix}
                      />
                    </div>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>

      <div className="flex items-center justify-end gap-2">
        {dirty ? (
          <span
            className="text-[12px] text-muted-foreground tabular-nums"
            data-testid={`${testIdPrefix}-dirty`}
          >
            {changes.length} unsaved{" "}
            {changes.length === 1 ? "change" : "changes"}
          </span>
        ) : null}
        <Button
          variant="outline"
          size="sm"
          disabled={!dirty}
          data-testid={`${testIdPrefix}-reset`}
          onClick={() => {
            setDraft(saved)
            setErrors([])
          }}
        >
          Reset
        </Button>
        <Button
          size="sm"
          disabled={!dirty}
          data-testid={`${testIdPrefix}-review`}
          onClick={() => setReviewing(true)}
        >
          <SaveIcon data-icon="inline-start" />
          Review changes
        </Button>
      </div>

      <Dialog
        open={reviewing}
        onOpenChange={(open) => !saving && setReviewing(open)}
      >
        <DialogContent
          data-testid={`${testIdPrefix}-diff`}
          className="sm:max-w-lg"
        >
          <DialogHeader>
            <DialogTitle>Save role slots?</DialogTitle>
            <DialogDescription>
              {changes.length} {changes.length === 1 ? "change" : "changes"}{" "}
              take effect on the next turn.
            </DialogDescription>
          </DialogHeader>
          <DiffList changes={changes} prefix={testIdPrefix} />
          <DialogFooter>
            <Button
              variant="outline"
              disabled={saving}
              onClick={() => setReviewing(false)}
            >
              Back
            </Button>
            <Button
              disabled={saving}
              data-testid={`${testIdPrefix}-confirm`}
              onClick={() => void confirm()}
            >
              {saving ? <Spinner data-icon="inline-start" /> : null}
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function PingResult({
  state,
  role,
  prefix,
}: {
  state: PingState | undefined
  role: string
  prefix: string
}) {
  if (!state || state.status === "pending") return null
  if (state.status === "unavailable")
    return (
      <p
        data-testid={`${prefix}-ping-${role}`}
        className="max-w-40 text-[11px] text-muted-foreground"
      >
        Not supported by this server build
      </p>
    )
  const { ok, latencyMs, message } = state.result
  return (
    <p
      role="status"
      data-testid={`${prefix}-ping-${role}`}
      data-ok={ok}
      className={
        ok
          ? "text-[11px] text-success tabular-nums"
          : "max-w-40 text-[11px] text-destructive"
      }
    >
      {ok ? `OK · ${latencyMs} ms` : message}
    </p>
  )
}

function DiffList({
  changes,
  prefix,
}: {
  changes: ProfileChange[]
  prefix: string
}) {
  return (
    <ul
      role="list"
      data-testid={`${prefix}-diff-list`}
      className="flex max-h-72 flex-col overflow-y-auto border text-[12px]"
    >
      {changes.map((c) => (
        <li
          key={`${c.role}-${c.field}`}
          className="flex items-center justify-between gap-3 border-b px-3 py-1.5 last:border-b-0"
        >
          <span className="font-medium">
            {roleLabel(c.role)}
            <span className="text-muted-foreground"> · {c.field}</span>
          </span>
          <span className="tabular-nums">
            <del className="text-muted-foreground">{c.from}</del>
            <span aria-hidden> → </span>
            <span className="sr-only"> changed to </span>
            <ins className="no-underline">{c.to}</ins>
          </span>
        </li>
      ))}
    </ul>
  )
}
