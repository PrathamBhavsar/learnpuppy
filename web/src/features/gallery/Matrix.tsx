import { ArrowRightIcon, SearchIcon } from "lucide-react"

import { MasteryRing } from "@/components/app/mastery-ring"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { DirectionProvider } from "@/components/ui/direction"
import { Input } from "@/components/ui/input"
import { Progress } from "@/components/ui/progress"
import { Switch } from "@/components/ui/switch"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { Direction, ResolvedTheme } from "@/lib/theme"

import { Section } from "./Section"

const QUADRANTS: { theme: ResolvedTheme; dir: Direction }[] = [
  { theme: "dark", dir: "ltr" },
  { theme: "dark", dir: "rtl" },
  { theme: "light", dir: "ltr" },
  { theme: "light", dir: "rtl" },
]

const COPY: Record<
  Direction,
  {
    title: string
    body: string
    search: string
    open: string
    due: string
    fresh: string
    error: string
  }
> = {
  ltr: {
    title: "Rust systems",
    body: "812 nodes, 1.2k chains.",
    search: "Search nodes",
    open: "Open workspace",
    due: "3 due",
    fresh: "new",
    error: "error",
  },
  rtl: {
    title: "أنظمة روست",
    body: "٨١٢ عقدة و١٫٢ ألف سلسلة.",
    search: "ابحث في العقد",
    open: "افتح مساحة العمل",
    due: "٣ مستحقة",
    fresh: "جديد",
    error: "خطأ",
  },
}

/** A theme + direction scope that works without touching <html>; overlays still follow the real theme. */
function Quadrant({ theme, dir }: { theme: ResolvedTheme; dir: Direction }) {
  const copy = COPY[dir]
  return (
    <DirectionProvider dir={dir}>
      <div
        className={`${theme} flex flex-col gap-3 border bg-background p-4 text-foreground`}
        dir={dir}
        data-testid={`matrix-${theme}-${dir}`}
        style={{ colorScheme: theme }}
      >
        <span className="text-[11px] text-muted-foreground">
          {theme} / {dir.toUpperCase()}
        </span>
        <Card>
          <CardHeader>
            <CardTitle>{copy.title}</CardTitle>
            <CardDescription>{copy.body}</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-3">
            <div className="relative">
              <SearchIcon
                aria-hidden
                className="pointer-events-none absolute inset-y-0 start-2 my-auto size-3.5 text-muted-foreground"
              />
              <Input
                aria-label={copy.search}
                placeholder={copy.search}
                className="ps-7"
              />
            </div>
            <Tabs defaultValue="a">
              <TabsList>
                <TabsTrigger value="a">Graph</TabsTrigger>
                <TabsTrigger value="b">Review</TabsTrigger>
              </TabsList>
              <TabsContent value="a" className="sr-only">
                Graph
              </TabsContent>
              <TabsContent value="b" className="sr-only">
                Review
              </TabsContent>
            </Tabs>
            <div className="flex items-center gap-2">
              <Badge>{copy.due}</Badge>
              <Badge variant="secondary">{copy.fresh}</Badge>
              <Badge variant="destructive">{copy.error}</Badge>
              <MasteryRing value={0.7} size={24} />
              <Switch aria-label="Follow new nodes" defaultChecked />
            </div>
            <Progress value={58} aria-label="Progress" />
            <Button className="self-start">
              {copy.open}
              <ArrowRightIcon
                data-icon="inline-end"
                className="rtl:-scale-x-100"
              />
            </Button>
          </CardContent>
        </Card>
      </div>
    </DirectionProvider>
  )
}

export function Matrix() {
  return (
    <Section
      id="matrix"
      title="Light / dark x LTR / RTL"
      className="[&>*]:min-w-72 [&>*]:flex-1"
    >
      {QUADRANTS.map((q) => (
        <Quadrant key={`${q.theme}-${q.dir}`} {...q} />
      ))}
    </Section>
  )
}
