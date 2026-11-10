import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { MethodSetting } from "@/api/learning-types"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useLearningActions, useMethodSettings } from "@/api/queries/learning"
import { toastError } from "@/lib/toast"

import { SettingRow } from "./SettingRow"

/** Plain-language names for the plan's numbers. Every one is a setting, not a fact. */
const COPY: Record<string, { label: string; hint: string }> = {
  bout_limit_minutes: {
    label: "Bout length (min)",
    hint: "Active minutes before the session must close.",
  },
  gap_between_bouts_minutes: {
    label: "Gap between bouts (min)",
    hint: "Rest enforced after a bout ends.",
  },
  daily_limit_minutes: {
    label: "Daily limit (min)",
    hint: "Active study minutes allowed per day.",
  },
  idle_rewind_minutes: {
    label: "Idle cutoff (min)",
    hint: "A silence longer than this stops counting as study time.",
  },
  gap_pause_seconds: {
    label: "Gap pause length (s)",
    hint: "A blank pause between steps. 0 turns pauses off.",
  },
  gap_pause_every_minutes: {
    label: "Gap pause every (min)",
    hint: "About one pause per this many active minutes. 0 turns pauses off.",
  },
  mixed_pass_every: {
    label: "Mixed quiz every",
    hint: "Every n-th quiz draws from earlier lessons. 0 turns it off.",
  },
  target_error_rate_pct: {
    label: "Target error rate (%)",
    hint: "Difficulty aims for this share of wrong answers.",
  },
  error_band_low_pct: {
    label: "Too easy below (%)",
    hint: "Under this error rate, items get harder.",
  },
  error_band_high_pct: {
    label: "Too hard above (%)",
    hint: "Over this error rate, items get easier.",
  },
  attempts_before_parking: {
    label: "Attempts before parking",
    hint: "Failed tries on one item before it is parked until Close.",
  },
  probe_strands: {
    label: "Probe strands",
    hint: "Parts of the topic the starting quiz checks.",
  },
  probe_questions_per_strand: {
    label: "Probe questions per strand",
    hint: "At most this many questions in each part.",
  },
  probe_questions_max: {
    label: "Probe questions in total",
    hint: "Upper limit for the whole starting quiz.",
  },
  strong_calls_per_hour: {
    label: "Strong model calls per hour",
    hint: "Budget for the expensive model in one hour of study.",
  },
}

function Row({ ws, s }: { ws: string; s: MethodSetting }) {
  const { setMethodSetting } = useLearningActions(ws)
  const [draft, setDraft] = useState(String(s.value))
  const [shown, setShown] = useState(s.value)
  // A saved value from the server replaces the draft (adjusted while rendering, not in an effect).
  if (s.value !== shown) {
    setShown(s.value)
    setDraft(String(s.value))
  }
  const copy = COPY[s.name] ?? { label: s.name, hint: "" }
  const save = (value: number) =>
    setMethodSetting.mutate(
      { name: s.name, value },
      {
        onError: (e) => {
          setDraft(String(s.value))
          toastError(e, `Could not save ${copy.label.toLowerCase()}`)
        },
      }
    )
  const commit = () => {
    const n = Number(draft)
    if (draft.trim() === "" || !Number.isInteger(n))
      return setDraft(String(s.value))
    if (n !== s.value) save(n)
  }
  const id = `method-${s.name}`
  return (
    <SettingRow
      label={copy.label}
      description={`${copy.hint} Allowed ${s.min} to ${s.max}; default ${s.default}.`}
      htmlFor={id}
    >
      <div className="flex items-center gap-2">
        {s.value !== s.default ? (
          <Button
            variant="ghost"
            size="sm"
            disabled={setMethodSetting.isPending}
            onClick={() => save(s.default)}
          >
            Reset
          </Button>
        ) : null}
        <Input
          id={id}
          data-testid={id}
          type="number"
          inputMode="numeric"
          min={s.min}
          max={s.max}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit()
          }}
          className="w-24"
        />
      </div>
    </SettingRow>
  )
}

/** The learning method's limits and defaults for one workspace. */
export function MethodSettingsSection({ ws }: { ws: string }) {
  const q = useMethodSettings(ws)
  if (q.isError)
    return (
      <ErrorState
        message={toApiError(q.error).message}
        onRetry={() => q.refetch()}
      />
    )
  if (!q.data) return <SkeletonBlock className="h-24" />
  return (
    <div data-testid="method-settings" className="flex flex-col divide-y">
      {q.data.settings.map((s) => (
        <Row key={s.name} ws={ws} s={s} />
      ))}
    </div>
  )
}
