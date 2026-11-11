import { createFileRoute, useNavigate } from "@tanstack/react-router"

import { TextbookScreen } from "@/features/textbook/TextbookScreen"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  const { book, upload } = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })
  usePageTitle("Textbooks", useWorkspaceName(ws))
  return (
    <TextbookScreen
      ws={ws}
      book={book}
      focusUpload={upload}
      onOpen={(next) => void navigate({ search: { book: next } })}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/textbook")({
  component: Screen,
  validateSearch: (
    s: Record<string, unknown>
  ): { book?: string; upload?: boolean } => ({
    book: typeof s["book"] === "string" && s["book"] ? s["book"] : undefined,
    upload:
      s["upload"] === "1" || s["upload"] === 1 || s["upload"] === true
        ? true
        : undefined,
  }),
})
