import { useState } from "react"

import type { SettingsDoc } from "@/api/pending-types"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"

import { SettingRow } from "./SettingRow"
import { useSettingsPatch } from "./preferences"

export function BehaviorCard({ doc }: { doc: SettingsDoc }) {
  const { patch } = useSettingsPatch()
  const [cap, setCap] = useState(doc.concurrencyCap)
  const [seen, setSeen] = useState(doc.concurrencyCap)
  if (seen !== doc.concurrencyCap) {
    setSeen(doc.concurrencyCap)
    setCap(doc.concurrencyCap)
  }

  return (
    <div data-testid="settings-behavior" className="flex flex-col divide-y">
      <SettingRow
        label="Confirm destructive actions"
        description="Ask before deleting nodes, chats or workspaces."
        htmlFor="confirm-destructive"
      >
        <Switch
          id="confirm-destructive"
          checked={doc.ui.confirmDestructive}
          data-testid="confirm-destructive"
          onCheckedChange={(on) =>
            patch({ ui: { confirmDestructive: on } }, "this setting")
          }
        />
      </SettingRow>

      <SettingRow
        label="Concurrency cap"
        description="Model calls that may run at once, from 1 to 6."
      >
        <div className="flex w-56 items-center gap-3">
          <Slider
            min={1}
            max={6}
            step={1}
            value={[cap]}
            aria-label="Concurrency cap"
            data-testid="concurrency-cap"
            onValueChange={([value]) => value !== undefined && setCap(value)}
            onValueCommit={([value]) =>
              value !== undefined &&
              value !== doc.concurrencyCap &&
              patch({ concurrencyCap: value }, "the concurrency cap")
            }
          />
          <output
            data-testid="concurrency-value"
            className="w-4 text-end text-[13px] font-medium tabular-nums"
          >
            {cap}
          </output>
        </div>
      </SettingRow>

      <SettingRow
        label="Open the browser on start"
        description="When you run learnpuppy serve."
        htmlFor="auto-open"
      >
        <Switch
          id="auto-open"
          checked={doc.ui.autoOpenBrowser}
          data-testid="auto-open"
          onCheckedChange={(on) =>
            patch({ ui: { autoOpenBrowser: on } }, "this setting")
          }
        />
      </SettingRow>
    </div>
  )
}
