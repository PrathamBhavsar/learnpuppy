import { createFileRoute } from "@tanstack/react-router"
import { DownloadIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  return (
    <PlaceholderScreen
      testId="screen-ingest"
      title="Ingest"
      description="Sources scanned for sessions, decisions and commits."
      icon={DownloadIcon}
      emptyTitle="No sources selected"
      emptyDescription="Pick the tools and project folders to scan, then run an ingest."
      actionLabel="Choose sources"
      workspace={useWorkspaceName(ws)}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/ingest")({
  component: Screen,
})
