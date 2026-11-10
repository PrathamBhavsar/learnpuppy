import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Textarea } from "@/components/ui/textarea"

/** F04: the first bout of the day asks how you slept. Self-reported only. */
export function SleepCheckin({
  busy,
  suggestReviewOnly,
  onSubmit,
}: {
  busy: boolean
  /** `null` before submission; the server's verdict once it has one. */
  suggestReviewOnly: boolean | null
  onSubmit: (hours: number, note: string) => void
}) {
  const [hours, setHours] = useState("")
  const [note, setNote] = useState("")
  const parsed = Number(hours)
  const valid =
    hours.trim() !== "" &&
    Number.isFinite(parsed) &&
    parsed >= 0 &&
    parsed <= 24

  if (suggestReviewOnly !== null) {
    return (
      <section data-testid="step-checkin-done" className="flex flex-col gap-2">
        <h2 className="font-heading text-[16px] font-medium">Thanks</h2>
        <p className="text-[13px]">
          {suggestReviewOnly
            ? "Short night. Today might go better as a review-only day."
            : "Logged. On to the bout."}
        </p>
      </section>
    )
  }

  return (
    <section data-testid="step-checkin" className="flex flex-col gap-3">
      <h2 className="font-heading text-[16px] font-medium">
        How did you sleep?
      </h2>
      <div className="flex flex-col gap-1">
        <Label htmlFor="sleep-hours">Hours of sleep</Label>
        <Input
          id="sleep-hours"
          type="number"
          inputMode="decimal"
          min={0}
          max={24}
          step={0.5}
          value={hours}
          onChange={(e) => setHours(e.target.value)}
          className="w-24"
        />
      </div>
      <Textarea
        value={note}
        onChange={(e) => setNote(e.target.value)}
        aria-label="Anything else about last night"
        placeholder="Optional note"
        rows={2}
      />
      <Button
        className="self-start"
        disabled={!valid || busy}
        onClick={() => onSubmit(parsed, note.trim())}
      >
        Continue
      </Button>
    </section>
  )
}
