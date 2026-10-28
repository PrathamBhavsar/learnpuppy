"use client"

import * as React from "react"
import { HugeiconsIcon } from "@hugeicons/react"
import {
  Brain02Icon,
  FlashIcon,
  Home01Icon,
  Search01Icon,
  Settings01Icon,
  StarIcon,
  Tick02Icon,
} from "@hugeicons/core-free-icons"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Progress } from "@/components/ui/progress"
import { Slider } from "@/components/ui/slider"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"

const SWATCHES: { name: string; token: string; hex: string }[] = [
  { name: "background", token: "background", hex: "#0a0810" },
  { name: "foreground", token: "foreground", hex: "#e9e4f5" },
  { name: "primary / ring", token: "primary", hex: "#a78bfa" },
  { name: "accent", token: "accent", hex: "#d8b4fe" },
  { name: "muted-foreground", token: "muted-foreground", hex: "#b9a7e6" },
  { name: "destructive", token: "destructive", hex: "#ee7d92" },
  { name: "success", token: "success", hex: "#86d6a2" },
  { name: "warning", token: "warning", hex: "#f0c560" },
  { name: "info", token: "info", hex: "#7db8f0" },
  { name: "chart-3", token: "chart-3", hex: "#5fd0c5" },
  { name: "chart-4", token: "chart-4", hex: "#f6a55c" },
  { name: "border / input", token: "border", hex: "#6b6577" },
]

const ICONS = [
  { name: "Home01", icon: Home01Icon },
  { name: "Search01", icon: Search01Icon },
  { name: "Brain02", icon: Brain02Icon },
  { name: "Settings01", icon: Settings01Icon },
  { name: "Star", icon: StarIcon },
  { name: "Flash", icon: FlashIcon },
  { name: "Tick02", icon: Tick02Icon },
]

function Section({
  id,
  title,
  children,
}: {
  id: string
  title: string
  children: React.ReactNode
}) {
  return (
    <section data-testid={`section-${id}`} className="flex flex-col gap-3">
      <h2 className="text-sm font-medium text-muted-foreground">{title}</h2>
      {children}
    </section>
  )
}

export default function Page() {
  const [on, setOn] = React.useState(true)
  const [level, setLevel] = React.useState<number>(60)

  return (
    <main
      data-testid="gallery"
      className="mx-auto flex max-w-4xl flex-col gap-10 p-6 md:p-10"
    >
      <header className="flex flex-col gap-1">
        <h1 data-testid="gallery-title" className="text-2xl font-medium">
          LearnPuppy design system lab
        </h1>
        <p className="text-sm text-muted-foreground">
          shadcn base-lyra preset, Geist Mono, Hugeicons, torlink palette.
        </p>
      </header>

      <Section id="palette" title="Palette">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4">
          {SWATCHES.map((s) => (
            <div
              key={s.name}
              data-testid={`swatch-${s.token}`}
              className="flex flex-col gap-1.5 text-xs"
            >
              <div
                className="h-12 border"
                style={{ backgroundColor: `var(--${s.token})` }}
              />
              <span>{s.name}</span>
              <span className="text-muted-foreground">{s.hex}</span>
            </div>
          ))}
        </div>
      </Section>

      <Section id="icons" title="Hugeicons">
        <div className="flex flex-wrap gap-4">
          {ICONS.map((i) => (
            <div
              key={i.name}
              data-testid={`icon-${i.name}`}
              className="flex flex-col items-center gap-1 text-xs text-muted-foreground"
            >
              <HugeiconsIcon icon={i.icon} className="size-6 text-primary" />
              {i.name}
            </div>
          ))}
        </div>
      </Section>

      <Section id="button" title="Button">
        <div className="flex flex-wrap items-center gap-2">
          {(
            [
              "default",
              "secondary",
              "outline",
              "ghost",
              "destructive",
              "link",
            ] as const
          ).map((v) => (
            <Button key={v} variant={v} data-testid={`button-${v}`}>
              {v}
            </Button>
          ))}
          <Button size="icon" aria-label="Settings" data-testid="button-icon">
            <HugeiconsIcon icon={Settings01Icon} />
          </Button>
        </div>
      </Section>

      <Section id="badge" title="Badge">
        <div className="flex flex-wrap items-center gap-2">
          {(
            ["default", "secondary", "outline", "destructive", "ghost"] as const
          ).map((v) => (
            <Badge key={v} variant={v} data-testid={`badge-${v}`}>
              {v}
            </Badge>
          ))}
          <Badge
            variant="outline"
            data-testid="chip-graph-node"
            className="border-primary text-primary"
          >
            <HugeiconsIcon icon={Brain02Icon} data-icon="inline-start" />
            graph node
          </Badge>
        </div>
      </Section>

      <Section id="card" title="Card">
        <Card data-testid="card-demo" className="max-w-sm">
          <CardHeader>
            <CardTitle>Spaced repetition</CardTitle>
            <CardDescription>12 cards due today</CardDescription>
          </CardHeader>
          <CardContent className="text-sm text-muted-foreground">
            FSRS schedules the next review from your recall rating.
          </CardContent>
        </Card>
      </Section>

      <Section id="tabs" title="Tabs">
        <Tabs defaultValue="graph" data-testid="tabs-demo">
          <TabsList>
            <TabsTrigger value="graph" data-testid="tab-graph">
              Graph
            </TabsTrigger>
            <TabsTrigger value="plan" data-testid="tab-plan">
              Plan
            </TabsTrigger>
            <TabsTrigger value="digest" data-testid="tab-digest">
              Digest
            </TabsTrigger>
          </TabsList>
          <TabsContent value="graph" className="text-sm">
            Knowledge graph canvas.
          </TabsContent>
          <TabsContent value="plan" className="text-sm">
            Plan DAG viewport.
          </TabsContent>
          <TabsContent value="digest" className="text-sm">
            FSRS digest.
          </TabsContent>
        </Tabs>
      </Section>

      <Section id="dialog" title="Dialog and Tooltip">
        <div className="flex flex-wrap items-center gap-2">
          <Dialog>
            <DialogTrigger
              render={
                <Button variant="outline" data-testid="dialog-trigger" />
              }
            >
              Open dialog
            </DialogTrigger>
            <DialogContent data-testid="dialog-content">
              <DialogHeader>
                <DialogTitle>Accounts and slots</DialogTitle>
                <DialogDescription>
                  Each account is its own rate-limit bucket.
                </DialogDescription>
              </DialogHeader>
            </DialogContent>
          </Dialog>
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Flash"
                  data-testid="tooltip-trigger"
                />
              }
            >
              <HugeiconsIcon icon={FlashIcon} />
            </TooltipTrigger>
            <TooltipContent data-testid="tooltip-content">
              Quick review
            </TooltipContent>
          </Tooltip>
        </div>
      </Section>

      <Section id="progress" title="Progress (mastery)">
        <div className="flex max-w-sm flex-col gap-3">
          <Progress value={25} data-testid="progress-novice" />
          <Progress value={55} data-testid="progress-learning" />
          <Progress value={90} data-testid="progress-mastered" />
        </div>
      </Section>

      <Section id="slider" title="Slider">
        <div className="flex max-w-sm flex-col gap-2">
          <Slider
            value={[level]}
            onValueChange={(v) => setLevel(Array.isArray(v) ? v[0] : v)}
            data-testid="slider-demo"
          />
          <span className="text-xs text-muted-foreground">{level}</span>
        </div>
      </Section>

      <Section id="switch" title="Switch">
        <div className="flex items-center gap-2 text-sm">
          <Switch
            checked={on}
            onCheckedChange={setOn}
            aria-label="Toggle sync"
            data-testid="switch-demo"
          />
          <span>{on ? "Sync on" : "Sync off"}</span>
        </div>
      </Section>

      <Section id="command" title="Command">
        <Command data-testid="command-demo" className="max-w-sm border">
          <CommandInput placeholder="Search" data-testid="command-input" />
          <CommandList>
            <CommandEmpty>No results.</CommandEmpty>
            <CommandGroup heading="Go to">
              <CommandItem data-testid="command-item-graph">
                <HugeiconsIcon icon={Brain02Icon} />
                Knowledge graph
              </CommandItem>
              <CommandItem data-testid="command-item-home">
                <HugeiconsIcon icon={Home01Icon} />
                Classroom
              </CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </Section>
    </main>
  )
}
