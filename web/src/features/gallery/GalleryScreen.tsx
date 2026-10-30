import { PageHeader } from "@/components/app/page-header"
import { useDirectionPref, useTheme } from "@/hooks/use-theme"
import { usePageTitle } from "@/hooks/use-page-title"

import { AppParts } from "./AppParts"
import { Controls } from "./Controls"
import { Data } from "./Data"
import { Matrix } from "./Matrix"
import { Overlays } from "./Overlays"
import { Palette } from "./Palette"

export function GalleryScreen() {
  usePageTitle("Gallery")
  const { resolvedTheme } = useTheme()
  const { direction } = useDirectionPref()
  return (
    <div data-testid="gallery" className="flex flex-col gap-10 p-6">
      <PageHeader
        title="Component gallery"
        description={`Every component the app uses. Showing ${resolvedTheme} theme, ${direction.toUpperCase()} layout; use the topbar to switch.`}
      />
      <Matrix />
      <Controls />
      <Overlays />
      <Data />
      <AppParts />
      <Palette />
    </div>
  )
}
