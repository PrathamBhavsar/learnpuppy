import { useEffect, useRef, useState } from "react"

import type { CloseReport, ReviewDate } from "@/api/method-types"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { toastError } from "@/lib/toast"

import { type MethodActions, useReviewDates } from "./method-api"

const HOUR = 3_600_000
const DAY = 24 * HOUR

/** F29: a blank box first, no notes visible; code then shows which key facts were left out. */
function BrainDump({
  actions,
  onDone,
}: {
  actions: MethodActions
  onDone: () => void
}) {
  const [text, setText] = useState("")
  const [missed, setMissed] = useState<string[] | null>(null)
  const { brainDump } = actions
  return (
    <section data-testid="course-braindump" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">Brain dump</h2>
      <p className="text-[13px]">
        What were the key points? Write what you remember, without looking back.
      </p>
      <Textarea
        value={text}
        disabled={missed !== null}
        onChange={(e) => setText(e.target.value)}
        aria-label="Your brain dump"
        rows={6}
      />
      {missed === null ? (
        <Button
          className="self-start"
          disabled={brainDump.isPending || text.trim() === ""}
          onClick={() =>
            brainDump.mutate(text.trim(), {
              onSuccess: (r) => setMissed(r.missed),
              onError: (e) => toastError(e, "Could not check your brain dump"),
            })
          }
        >
          Check
        </Button>
      ) : (
        <div className="flex flex-col gap-2">
          <div data-testid="braindump-missed" className="text-[13px]">
            {missed.length ? (
              <>
                <p>You left out:</p>
                <ul className="list-disc ps-5">
                  {missed.map((m) => (
                    <li key={m}>{m}</li>
                  ))}
                </ul>
              </>
            ) : (
              <p>You covered every key point.</p>
            )}
          </div>
          <Button className="self-start" onClick={onDone}>
            Finish
          </Button>
        </div>
      )}
    </section>
  )
}

/** F32: one batched screen of proposed first tests; the learner may move any, never past a day. */
function ReviewDates({
  ws,
  id,
  actions,
}: {
  ws: string
  id: string
  actions: MethodActions
}) {
  const q = useReviewDates(ws, id, true)
  const [hours, setHours] = useState<Record<string, number>>({})
  const [saved, setSaved] = useState(false)
  const [now] = useState(() => Date.now())
  const dates: ReviewDate[] = q.data ?? []
  if (q.isPending) return <Spinner />
  if (dates.length === 0) return null
  const hoursFor = (d: ReviewDate) =>
    hours[d.cardId] ??
    Math.min(24, Math.max(0, Math.round((d.due - now) / HOUR)))
  return (
    <section data-testid="course-review-dates" className="flex flex-col gap-2">
      <h3 className="font-heading text-[15px] font-medium">First tests</h3>
      <ul className="flex flex-col gap-1 text-[13px]">
        {dates.map((d) => (
          <li key={d.cardId} className="flex items-center gap-2">
            <span className="flex-1">{d.label}</span>
            <label className="flex items-center gap-1">
              <span className="text-muted-foreground">in</span>
              <input
                type="number"
                min={0}
                max={24}
                className="w-14 rounded border bg-background px-1"
                aria-label={`Hours until test: ${d.label}`}
                value={hoursFor(d)}
                disabled={saved}
                onChange={(e) =>
                  setHours({
                    ...hours,
                    [d.cardId]: Math.min(
                      24,
                      Math.max(0, Number(e.target.value))
                    ),
                  })
                }
              />
              <span className="text-muted-foreground">h</span>
            </label>
          </li>
        ))}
      </ul>
      {saved ? (
        <p className="text-[13px]">Dates saved. They show in Review.</p>
      ) : (
        <Button
          className="self-start"
          disabled={actions.saveDates.isPending}
          onClick={() =>
            actions.saveDates.mutate(
              {
                dates: dates.map((d) => ({
                  cardId: d.cardId,
                  due: now + Math.min(DAY, hoursFor(d) * HOUR),
                })),
              },
              {
                onSuccess: () => setSaved(true),
                onError: (e) => toastError(e, "Could not save the dates"),
              }
            )
          }
        >
          Save dates
        </Button>
      )}
    </section>
  )
}

/** Brain dump first, then the server ends the session and returns the report and review dates. */
export function Close({
  ws,
  id,
  actions,
  report,
  onReport,
}: {
  ws: string
  id: string
  actions: MethodActions
  report: CloseReport | null
  onReport: (r: CloseReport) => void
}) {
  const [dumped, setDumped] = useState(false)
  const sent = useRef(false)
  const { close } = actions
  useEffect(() => {
    if (!dumped || sent.current) return
    sent.current = true
    close.mutate(undefined, {
      onSuccess: onReport,
      onError: (e) => toastError(e, "Could not close the session"),
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dumped])
  if (!dumped)
    return <BrainDump actions={actions} onDone={() => setDumped(true)} />
  if (!report) return <Spinner />
  return (
    <section data-testid="course-close" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">Session finished</h2>
      <p className="text-[13px]">{report.countLine}</p>
      {report.takeaways.length ? (
        <ul className="list-disc ps-5 text-[13px]">
          {report.takeaways.map((t) => (
            <li key={t}>{t}</li>
          ))}
        </ul>
      ) : null}
      {report.focus.length ? (
        <p className="text-[13px]">Next time: {report.focus.join("; ")}</p>
      ) : null}
      {report.sureTotal > 0 ? (
        <p className="text-[13px] text-muted-foreground">
          Sure and wrong: {report.sureWrong} of {report.sureTotal}
        </p>
      ) : null}
      {report.resume ? (
        <p className="text-[13px]">Pick up at {report.resume}.</p>
      ) : null}
      <ReviewDates ws={ws} id={id} actions={actions} />
    </section>
  )
}
