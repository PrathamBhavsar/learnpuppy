import { createFileRoute } from "@tanstack/react-router"

import { GalleryScreen } from "@/features/gallery/GalleryScreen"

export const Route = createFileRoute("/_shell/gallery")({
  component: GalleryScreen,
})
