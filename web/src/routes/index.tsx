import { createFileRoute } from "@tanstack/react-router"

import { HomeScreen } from "@/features/workspaces/HomeScreen"

function Home() {
  const { new: newRequested } = Route.useSearch()
  return <HomeScreen newRequested={newRequested === true} />
}

export const Route = createFileRoute("/")({
  component: Home,
  validateSearch: (search: Record<string, unknown>): { new?: true } =>
    search["new"] === true || search["new"] === "1" || search["new"] === 1
      ? { new: true }
      : {},
})
