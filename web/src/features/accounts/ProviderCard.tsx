import { useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"

import { ConnectDialog } from "./ConnectDialog"
import {
  CONNECT_COMMAND,
  FREE_PROVIDERS,
  PROVIDER_BLURB,
  providerTitle,
} from "@/lib/accounts-model"

type ProviderCardProps = {
  provider: string
  onRescan: () => void
}

/** A provider with no login on this machine: what it is and how to connect it. */
export function ProviderCard({ provider, onRescan }: ProviderCardProps) {
  const [open, setOpen] = useState(false)
  const free = (FREE_PROVIDERS as readonly string[]).includes(provider)
  return (
    <li
      data-testid={`provider-card-${provider}`}
      data-status="not_connected"
      className="flex flex-col gap-2 border border-dashed bg-card p-4"
    >
      <div className="flex items-center gap-2">
        <span className="text-[13px] font-medium">
          {providerTitle(provider)}
        </span>
        {free ? <Badge variant="secondary">Free options</Badge> : null}
        <span className="ms-auto text-[12px] text-muted-foreground">
          Not connected
        </span>
      </div>
      <p className="text-[12px] text-muted-foreground">
        {PROVIDER_BLURB[provider]}
      </p>
      <code className="truncate text-[12px]">{CONNECT_COMMAND[provider]}</code>
      <Button
        variant="outline"
        size="sm"
        className="self-start"
        data-testid={`provider-howto-${provider}`}
        onClick={() => setOpen(true)}
      >
        How to connect
      </Button>
      <ConnectDialog
        provider={open ? provider : null}
        onOpenChange={setOpen}
        onRescan={() => {
          setOpen(false)
          onRescan()
        }}
      />
    </li>
  )
}
