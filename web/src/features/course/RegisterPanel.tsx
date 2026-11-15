import { Spinner } from "@/components/ui/spinner"

import { useRegister } from "./method-api"

const STATUS = {
  open: "Still open",
  "retested-ok": "Fixed on retest",
  recurring: "Keeps coming back",
} as const

/** Wrong beliefs the learner has shown, with where each stands. */
export function RegisterPanel({ ws, id }: { ws: string; id: string }) {
  const rows = useRegister(ws, id, true)
  if (rows.isPending) return <Spinner />
  if (!rows.data?.length)
    return <p className="text-[13px]">No misconceptions recorded yet.</p>
  return (
    <ul data-testid="course-register" className="flex flex-col gap-2">
      {rows.data.map((r) => (
        <li key={r.item} className="border p-3 text-[13px]">
          <p>{r.belief}</p>
          <p className="text-muted-foreground">
            {STATUS[r.status]}, missed {r.misses}{" "}
            {r.misses === 1 ? "time" : "times"}
          </p>
        </li>
      ))}
    </ul>
  )
}
