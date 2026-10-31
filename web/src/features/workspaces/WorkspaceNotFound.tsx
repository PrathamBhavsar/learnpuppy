import { Link } from "@tanstack/react-router"
import { CompassIcon } from "lucide-react"

import { useWorkspaces } from "@/api/queries/workspaces"
import { EmptyState } from "@/components/app/empty-state"
import { Button } from "@/components/ui/button"
import { usePageTitle } from "@/hooks/use-page-title"

import { openProps } from "./model"
import { WorkspaceMark } from "./WorkspaceAvatar"

/** Unknown, archived or deleted `:ws` (docs/web/02 §Routing): never a blank screen, always a way out. */
export function WorkspaceNotFound() {
  usePageTitle("Workspace not found")
  const workspaces = useWorkspaces(false).data ?? []
  return (
    <div
      className="flex min-h-full flex-col items-center justify-center gap-4 p-6"
      data-testid="workspace-not-found"
    >
      <EmptyState
        icon={CompassIcon}
        title="Workspace not found"
        description="It may have been deleted, archived or moved to the trash."
        focusAction={workspaces.length === 0}
        action={
          <div className="flex gap-2">
            <Button asChild variant="outline">
              <Link to="/" data-testid="not-found-home">
                All workspaces
              </Link>
            </Button>
            <Button asChild variant="ghost">
              <Link to="/trash" data-testid="not-found-trash">
                Open trash
              </Link>
            </Button>
          </div>
        }
        className="py-6"
      />
      {workspaces.length > 0 ? (
        <nav aria-label="Open a workspace" className="w-full max-w-sm">
          <ul
            role="list"
            className="flex flex-col"
            data-testid="not-found-picker"
          >
            {workspaces.map((w) => (
              <li key={w.id}>
                <Link
                  {...openProps(w.id)}
                  data-testid={`not-found-option-${w.id}`}
                  className="flex items-center gap-2 px-2 py-1.5 text-[13px] hover:bg-muted"
                >
                  <WorkspaceMark
                    icon={w.icon}
                    hue={w.hue}
                    className="size-6 [&_svg]:size-3.5"
                  />
                  <span className="min-w-0 flex-1 truncate">{w.name}</span>
                </Link>
              </li>
            ))}
          </ul>
        </nav>
      ) : null}
    </div>
  )
}
