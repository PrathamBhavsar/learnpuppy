import { createFileRoute } from "@tanstack/react-router"

import { IngestScreen } from "@/features/ingest/IngestScreen"

function Screen() {
  const { ws } = Route.useParams()
  return <IngestScreen ws={ws} />
}

export const Route = createFileRoute("/_shell/w/$ws/ingest")({
  component: Screen,
})
