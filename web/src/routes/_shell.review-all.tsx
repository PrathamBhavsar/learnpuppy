import { createFileRoute } from "@tanstack/react-router"

import { ReviewAllScreen } from "@/features/review/review-all-screen"

export const Route = createFileRoute("/_shell/review-all")({
  component: ReviewAllScreen,
})
