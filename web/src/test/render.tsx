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

export function renderWithProviders(ui: ReactElement, options?: RenderOptions) {
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
export function renderApp(path: string) {
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
