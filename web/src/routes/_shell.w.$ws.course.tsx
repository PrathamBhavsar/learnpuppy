import { createFileRoute, useNavigate } from "@tanstack/react-router"

import { CourseScreen } from "@/features/course/CourseScreen"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  const { id } = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  usePageTitle("Courses", useWorkspaceName(ws))
  return (
    <CourseScreen
      ws={ws}
      id={id}
      onOpen={(next) => void navigate({ search: { id: next } })}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/course")({
  component: Screen,
  validateSearch: (s: Record<string, unknown>): { id?: string } => ({
    id: typeof s["id"] === "string" && s["id"] ? s["id"] : undefined,
  }),
})
