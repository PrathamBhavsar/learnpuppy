import { TOUR_KEY, useTour } from "@/components/app/tour/tour-store"
import { storage } from "@/lib/storage"
import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import {
  RouterProvider,
  createMemoryHistory,
  createRouter,
} from "@tanstack/react-router"
import { type RenderOptions, render } from "@testing-library/react"
import type { ReactElement } from "react"

import { AppProviders } from "@/components/app/providers"
import { routeTree } from "@/routeTree.gen"
import { usePalette } from "@/stores/palette"
import { useUi } from "@/stores/ui"

export function resetUi() {
  useTour.setState({ phase: "idle" })
  usePalette.setState({ open: false })
  useUi.setState({
    theme: "system",
    resolvedTheme: "light",
    direction: "ltr",
    reduceMotion: false,
    density: "comfortable",
    sidebarCollapsed: false,
    lastWorkspace: null,
  })
}

/** Most tests are not about the welcome tour: mark it done unless a test clears the key itself. */
function markTourDone() {
  if (storage.get(TOUR_KEY) === null) storage.set(TOUR_KEY, "done")
}

export function renderWithProviders(ui: ReactElement, options?: RenderOptions) {
  markTourDone()
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return render(
    <QueryClientProvider client={queryClient}>
      <AppProviders>{ui}</AppProviders>
    </QueryClientProvider>,
    options
  )
}

/** Render the real route tree at `path` inside an in-memory history. */
export function renderApp(path: string, options: { tour?: boolean } = {}) {
  if (!options.tour) markTourDone()
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createRouter({
    routeTree,
    context: { queryClient },
    history: createMemoryHistory({ initialEntries: [path] }),
  })
  const view = render(
    <QueryClientProvider client={queryClient}>
      <AppProviders>
        <RouterProvider router={router} />
      </AppProviders>
    </QueryClientProvider>
  )
  return { router, ...view }
}
