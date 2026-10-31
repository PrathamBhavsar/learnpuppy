import { QueryClientProvider } from "@tanstack/react-query"
import { RouterProvider } from "@tanstack/react-router"
import { StrictMode } from "react"
import { createRoot } from "react-dom/client"

import { AppProviders } from "@/components/app/providers"

import "./index.css"
import { queryClient, router } from "./router"

const root = document.getElementById("root")
if (!root) throw new Error("#root missing from index.html")

// `bun run dev:mock`: serve every endpoint from in-browser MSW handlers, no backend needed.
if (import.meta.env.MODE === "mock") {
  const { startMock } = await import("./test/mock/browser")
  await startMock(queryClient)
}

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <AppProviders>
        <RouterProvider router={router} />
      </AppProviders>
    </QueryClientProvider>
  </StrictMode>
)
