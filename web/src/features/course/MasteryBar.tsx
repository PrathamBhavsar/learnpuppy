import { useMastery } from "./method-api"

/** The server's own count line, e.g. "3 of 8 facts solid"; the client does no arithmetic. */
export function MasteryBar({
  ws,
  id,
  version,
}: {
  ws: string
  id: string
  version: number
}) {
  const line = useMastery(ws, id, version)
  if (!line.data) return null
  return (
    <p
      data-testid="course-mastery"
      className="text-[13px] text-muted-foreground"
    >
      {line.data}
    </p>
  )
}
