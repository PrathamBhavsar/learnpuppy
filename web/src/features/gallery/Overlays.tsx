import { toast } from "sonner"

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
} from "@/components/ui/context-menu"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuShortcut,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useEndSide } from "@/hooks/use-end-side"

import { Section, Specimen } from "./Section"

export function Overlays() {
  const endSide = useEndSide()
  return (
    <Section id="overlays" title="Overlays and menus">
      <Specimen label="Dialog">
        <Dialog>
          <DialogTrigger asChild>
            <Button variant="outline" data-testid="gallery-dialog-trigger">
              Open dialog
            </Button>
          </DialogTrigger>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Rename workspace</DialogTitle>
              <DialogDescription>
                The name is shown in the sidebar and the workspace list.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button>Save</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </Specimen>
      <Specimen label="Alert dialog">
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button variant="destructive">Delete workspace</Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Delete this workspace?</AlertDialogTitle>
              <AlertDialogDescription>
                It moves to the trash. You can restore it from there.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction>Delete</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      </Specimen>
      <Specimen label="Sheet (slides from the end edge)">
        <Sheet>
          <SheetTrigger asChild>
            <Button variant="outline" data-testid="gallery-sheet-trigger">
              Open sheet
            </Button>
          </SheetTrigger>
          <SheetContent side={endSide}>
            <SheetHeader>
              <SheetTitle>Node details</SheetTitle>
              <SheetDescription>
                Provenance, chains and linked cards.
              </SheetDescription>
            </SheetHeader>
          </SheetContent>
        </Sheet>
      </Specimen>
      <Specimen label="Popover">
        <Popover>
          <PopoverTrigger asChild>
            <Button variant="outline">Open popover</Button>
          </PopoverTrigger>
          <PopoverContent>
            Popovers fade and scale from their anchor.
          </PopoverContent>
        </Popover>
      </Specimen>
      <Specimen label="Tooltip">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="outline">Hover or focus me</Button>
          </TooltipTrigger>
          <TooltipContent>Tooltip text</TooltipContent>
        </Tooltip>
      </Specimen>
      <Specimen label="Dropdown menu">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline">Actions</Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuLabel>Workspace</DropdownMenuLabel>
            <DropdownMenuItem>
              Rename<DropdownMenuShortcut>R</DropdownMenuShortcut>
            </DropdownMenuItem>
            <DropdownMenuItem>Archive</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive">Delete</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </Specimen>
      <Specimen label="Context menu (secondary click)">
        <ContextMenu>
          <ContextMenuTrigger className="grid h-16 w-40 place-items-center border border-dashed text-[12px] text-muted-foreground">
            Secondary-click here
          </ContextMenuTrigger>
          <ContextMenuContent>
            <ContextMenuItem>Practice this</ContextMenuItem>
            <ContextMenuItem>Copy text</ContextMenuItem>
          </ContextMenuContent>
        </ContextMenu>
      </Specimen>
      <Specimen label="Toasts (sonner)">
        <Button variant="outline" onClick={() => toast.success("Saved")}>
          Success
        </Button>
        <Button
          variant="outline"
          onClick={() =>
            toast.error("Could not save", { description: "Request req_9f2a" })
          }
        >
          Error
        </Button>
      </Specimen>
      <Specimen label="Command (inline)" className="w-72">
        <Command className="border">
          <CommandInput placeholder="Search commands" />
          <CommandList>
            <CommandEmpty>No results.</CommandEmpty>
            <CommandGroup heading="Navigate">
              <CommandItem>Graph</CommandItem>
              <CommandItem>Review</CommandItem>
            </CommandGroup>
          </CommandList>
        </Command>
      </Specimen>
    </Section>
  )
}
