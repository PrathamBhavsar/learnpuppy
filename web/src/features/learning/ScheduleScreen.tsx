import { CalendarPlusIcon, SparklesIcon, Trash2Icon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { Schedule, TrackSlotsResult } from "@/api/learning-types"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"
import { toastError, toastSuccess } from "@/lib/toast"

import {
  useLearningActions,
  useSchedule,
  useTrackSlots,
} from "@/api/queries/learning"

const STATE_LABEL = { kept: "Kept", missed: "Missed", upcoming: "Upcoming" }
const STATE_VARIANT = {
  kept: "default",
  missed: "destructive",
  upcoming: "secondary",
} as const

/** F01: weekly slots, target 2 to 3 bouts a day on 5 days. `.ics` export is deferred past G4. */
export function ScheduleScreen({ ws }: { ws: string }) {
  usePageTitle("Schedule", useWorkspaceName(ws))
  const schedule = useSchedule(ws)
  const track = useTrackSlots(ws)

  if (schedule.isPending) {
    return (
      <div className="flex flex-col gap-4 p-6">
        <SkeletonBlock className="h-8 w-48" />
        <SkeletonBlock className="h-40 w-full" />
      </div>
    )
  }
  if (schedule.isError) {
    return (
      <div className="p-6">
        <ErrorState
          message={toApiError(schedule.error).message}
          onRetry={() => schedule.refetch()}
        />
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6 p-6" data-testid="schedule-screen">
      <PageHeader
        title="Schedule"
        description="Weekly study slots: 2 to 3 bouts a day, on 5 days."
      />
      {/* `key` resets the draft whenever the server record itself changes (e.g. after a save). */}
      <ScheduleForm
        key={JSON.stringify(schedule.data)}
        ws={ws}
        saved={schedule.data}
      />
      <TrackedSlots track={track.data} />
    </div>
  )
}

function ScheduleForm({ ws, saved }: { ws: string; saved: Schedule }) {
  const { setSchedule, suggestSlots } = useLearningActions(ws)
  const [wakeTime, setWakeTime] = useState(saved.wakeTime)
  const [slots, setSlots] = useState(saved.slots)
  const dirty =
    wakeTime !== saved.wakeTime || slots.join(",") !== saved.slots.join(",")

  const setSlot = (i: number, value: string) =>
    setSlots(slots.map((s, j) => (j === i ? value : s)))
  const removeSlot = (i: number) => setSlots(slots.filter((_, j) => j !== i))
  const addSlot = () => setSlots([...slots, "09:00"])

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-[14px]">Wake time and slots</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-1">
          <Label htmlFor="wake-time">Wake time</Label>
          <Input
            id="wake-time"
            type="time"
            className="w-32"
            value={wakeTime}
            onChange={(e) => setWakeTime(e.target.value)}
          />
        </div>
        <div className="flex flex-col gap-2">
          <Label>Slots</Label>
          {slots.map((slot, i) => (
            <div key={i} className="flex items-center gap-2">
              <Input
                type="time"
                className="w-32"
                value={slot}
                onChange={(e) => setSlot(i, e.target.value)}
                aria-label={`Slot ${i + 1}`}
              />
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Remove slot ${i + 1}`}
                onClick={() => removeSlot(i)}
              >
                <Trash2Icon className="size-4" />
              </Button>
            </div>
          ))}
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={addSlot}>
              <CalendarPlusIcon className="size-4" />
              Add slot
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={!wakeTime || suggestSlots.isPending}
              onClick={() =>
                suggestSlots.mutate(wakeTime, {
                  onSuccess: (r) => setSlots(r.slots),
                  onError: (error) =>
                    toastError(error, "Could not suggest slots"),
                })
              }
            >
              <SparklesIcon className="size-4" />
              Suggest
            </Button>
          </div>
        </div>
        <Button
          className="self-start"
          disabled={!dirty || setSchedule.isPending}
          onClick={() =>
            setSchedule.mutate(
              { wakeTime, slots },
              {
                onSuccess: () => toastSuccess("Schedule saved"),
                onError: (error) =>
                  toastError(error, "Could not save schedule"),
              }
            )
          }
        >
          Save
        </Button>
      </CardContent>
    </Card>
  )
}

function TrackedSlots({ track }: { track: TrackSlotsResult | undefined }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-[14px]">Kept and missed</CardTitle>
      </CardHeader>
      <CardContent>
        {track?.slots.length ? (
          <ul className="flex flex-col gap-1">
            {track.slots.map((r, i) => (
              <li
                key={i}
                className="flex items-center justify-between text-[13px]"
              >
                <span>
                  Day {r.day} · {r.slot}
                </span>
                <Badge variant={STATE_VARIANT[r.state]}>
                  {STATE_LABEL[r.state]}
                </Badge>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-[13px] text-muted-foreground">
            No tracked slots yet.
          </p>
        )}
      </CardContent>
    </Card>
  )
}
