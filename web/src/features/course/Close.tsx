import { useEffect, useRef } from "react"

import type { CloseReport } from "@/api/method-types"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import type { MethodActions } from "./method-api"

/** Ends the session on the server and shows the report it returns. */
export function Close({
  actions,
  report,
  onReport,
}: {
  actions: MethodActions
  report: CloseReport | null
  onReport: (r: CloseReport) => void
}) {
  const sent = useRef(false)
  const { close } = actions
  useEffect(() => {
    if (sent.current) return
    sent.current = true
    close.mutate(undefined, {
      onSuccess: onReport,
      onError: (e) => toastError(e, "Could not close the session"),
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
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
    </section>
  )
}
