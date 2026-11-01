import { ScreenGuide } from "@/components/app/info/info"
import { useTour } from "@/components/app/tour/tour-store"
import { CopyIcon, PlayIcon, Trash2Icon } from "lucide-react"

import { toApiError } from "@/api/errors"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { usePageTitle } from "@/hooks/use-page-title"
import { toastError, toastSuccess } from "@/lib/toast"
import { Link } from "@tanstack/react-router"

import { AppearanceCard } from "./AppearanceCard"
import { BehaviorCard } from "./BehaviorCard"
import { DiagnosticsCard } from "./DiagnosticsCard"
import { Section } from "./Section"
import { SettingRow } from "./SettingRow"
import { ShortcutsTable } from "./ShortcutsTable"
import { SETTINGS_GUIDE_PARTS, SETTINGS_INFO } from "./help"
import { useHealth } from "./health"
import { useSettingsPatch } from "./preferences"

function DataSection() {
  const health = useHealth()
  const dir = health.data?.dataDir
  const copy = async () => {
    if (!dir) return
    try {
      await navigator.clipboard.writeText(dir)
      toastSuccess("Path copied")
    } catch (error) {
      toastError(error, "Could not copy the path")
    }
  }
  return (
    <div data-testid="settings-data" className="flex flex-col divide-y">
      <SettingRow
        label="Data directory"
        description="Workspaces, ledgers and settings live here."
      >
        <div className="flex items-center gap-2">
          <code
            data-testid="data-dir"
            className="max-w-64 truncate text-[12px]"
            title={dir}
          >
            {dir ?? "…"}
          </code>
          <Button
            variant="outline"
            size="sm"
            disabled={!dir}
            onClick={() => void copy()}
            data-testid="copy-data-dir"
          >
            <CopyIcon data-icon="inline-start" />
            Copy
          </Button>
        </div>
      </SettingRow>
      <SettingRow
        label="Trash"
        description="Deleted workspaces wait here until you purge them."
      >
        <Button asChild variant="outline" size="sm">
          <Link to="/trash" data-testid="open-trash">
            <Trash2Icon data-icon="inline-start" />
            Open trash
          </Link>
        </Button>
      </SettingRow>
    </div>
  )
}

function HelpSection() {
  const replay = useTour((s) => s.replay)
  return (
    <SettingRow
      label="Welcome tour"
      description="A short guided walk through the app, pointing at the real buttons."
    >
      <Button
        variant="outline"
        size="sm"
        onClick={replay}
        data-testid="replay-tour"
      >
        <PlayIcon data-icon="inline-start" />
        Replay the tour
      </Button>
    </SettingRow>
  )
}

function AboutSection() {
  const health = useHealth()
  return (
    <dl
      data-testid="settings-about"
      className="grid grid-cols-2 gap-3 text-[13px] sm:grid-cols-3"
    >
      <div className="flex flex-col">
        <dt className="text-muted-foreground">Version</dt>
        <dd data-testid="about-version" className="font-medium tabular-nums">
          {health.data?.version ?? "…"}
        </dd>
      </div>
      <div className="flex flex-col">
        <dt className="text-muted-foreground">Engine</dt>
        <dd data-testid="about-engine" className="font-medium">
          {health.data
            ? health.data.engine === "faux"
              ? "Scripted (faux)"
              : "Real CLIs"
            : "…"}
        </dd>
      </div>
    </dl>
  )
}

export function SettingsScreen() {
  usePageTitle("Settings")
  const { settings } = useSettingsPatch()
  const doc = settings.data

  return (
    <section
      data-testid="screen-settings"
      aria-busy={settings.isPending}
      className="mx-auto flex min-h-full w-full max-w-3xl flex-col gap-6 p-6"
    >
      <PageHeader
        title="Settings"
        description="Appearance, behavior, shortcuts and data for this machine."
        actions={
          <ScreenGuide
            title="Settings"
            summary="Settings for this computer. They apply to every workspace."
            parts={SETTINGS_GUIDE_PARTS}
          />
        }
      />

      {settings.isPending ? (
        <div data-testid="settings-loading" className="flex flex-col gap-4">
          {[0, 1, 2].map((i) => (
            <SkeletonBlock key={i} className="h-40 w-full" />
          ))}
        </div>
      ) : settings.isError || !doc ? (
        <ErrorState
          title="Settings did not load"
          message={toApiError(settings.error).message}
          requestId={toApiError(settings.error).requestId}
          onRetry={() => settings.refetch()}
          className="border"
        />
      ) : (
        <>
          <Section
            id="appearance"
            title="Appearance"
            info={SETTINGS_INFO.appearance}
          >
            <AppearanceCard />
          </Section>
          <Section id="behavior" title="Behavior" info={SETTINGS_INFO.behavior}>
            <BehaviorCard doc={doc} />
          </Section>
          <Section
            id="shortcuts"
            title="Keyboard shortcuts"
            description="Press ? anywhere to open this list as a sheet."
            info={SETTINGS_INFO.shortcuts}
          >
            <ShortcutsTable />
          </Section>
          <Section id="data" title="Data" info={SETTINGS_INFO.data}>
            <DataSection />
            <DiagnosticsCard />
          </Section>
          <Section id="help" title="Help" info={SETTINGS_INFO.help}>
            <HelpSection />
          </Section>
          <Section id="about" title="About" info={SETTINGS_INFO.about}>
            <AboutSection />
          </Section>
        </>
      )}
    </section>
  )
}
