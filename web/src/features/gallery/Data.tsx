import { InfoIcon } from "lucide-react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { Progress } from "@/components/ui/progress"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { Button } from "@/components/ui/button"
import { Kbd as KbdKey } from "@/components/ui/kbd"

import { Section, Specimen } from "./Section"

export function Data() {
  return (
    <Section id="data" title="Data display and layout">
      <Specimen label="Card" className="w-72">
        <Card className="w-full">
          <CardHeader>
            <CardTitle>Rust systems</CardTitle>
            <CardDescription>812 nodes · 1.2k chains · 6 chats</CardDescription>
          </CardHeader>
          <CardContent className="text-[13px]">
            Last activity 2 hours ago.
          </CardContent>
          <CardFooter>
            <Button size="sm" variant="outline">
              Open
            </Button>
          </CardFooter>
        </Card>
      </Specimen>
      <Specimen label="Tabs" className="w-72">
        <Tabs defaultValue="chains" className="w-full">
          <TabsList>
            <TabsTrigger value="chains">Chains</TabsTrigger>
            <TabsTrigger value="cards">Cards</TabsTrigger>
            <TabsTrigger value="history">History</TabsTrigger>
          </TabsList>
          <TabsContent value="chains" className="text-[13px]">
            Chains linking this proposition.
          </TabsContent>
          <TabsContent value="cards" className="text-[13px]">
            Flashcards generated from it.
          </TabsContent>
          <TabsContent value="history" className="text-[13px]">
            Review history.
          </TabsContent>
        </Tabs>
      </Specimen>
      <Specimen label="Table" className="w-80">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Source</TableHead>
              <TableHead className="text-end">Nodes</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow>
              <TableCell>Claude</TableCell>
              <TableCell className="text-end tabular-nums">482</TableCell>
            </TableRow>
            <TableRow>
              <TableCell>Git</TableCell>
              <TableCell className="text-end tabular-nums">96</TableCell>
            </TableRow>
          </TableBody>
        </Table>
      </Specimen>
      <Specimen
        label="Progress and separator"
        className="w-56 flex-col items-stretch"
      >
        <Progress value={62} aria-label="Ingest progress" />
        <Separator />
        <Progress value={18} aria-label="Review progress" />
      </Specimen>
      <Specimen label="Avatar and kbd">
        <Avatar>
          <AvatarFallback>LP</AvatarFallback>
        </Avatar>
        <KbdKey>Esc</KbdKey>
      </Specimen>
      <Specimen label="Alert" className="w-80">
        <Alert>
          <InfoIcon aria-hidden />
          <AlertTitle>Engine unavailable</AlertTitle>
          <AlertDescription>
            No model account is ready. Open Accounts to fix it.
          </AlertDescription>
        </Alert>
      </Specimen>
      <Specimen label="Collapsible" className="w-56">
        <Collapsible className="w-full">
          <CollapsibleTrigger asChild>
            <Button variant="outline" size="sm">
              Details
            </Button>
          </CollapsibleTrigger>
          <CollapsibleContent className="pt-2 text-[13px] text-muted-foreground">
            Stability 4.2 · difficulty 6.1
          </CollapsibleContent>
        </Collapsible>
      </Specimen>
      <Specimen label="Scroll area" className="w-56">
        <ScrollArea className="h-24 w-full border">
          <ul className="p-2 text-[13px]">
            {Array.from({ length: 12 }, (_, i) => (
              <li key={i}>Proposition {i + 1}</li>
            ))}
          </ul>
        </ScrollArea>
      </Specimen>
      <Specimen label="Skeleton" className="w-56 flex-col items-stretch">
        <Skeleton className="h-4 w-full" />
        <Skeleton className="h-4 w-2/3" />
      </Specimen>
    </Section>
  )
}
