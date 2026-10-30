import {
  BoldIcon,
  ItalicIcon,
  MailIcon,
  SearchIcon,
  UnderlineIcon,
} from "lucide-react"
import { useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupInput,
} from "@/components/ui/input-group"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Slider } from "@/components/ui/slider"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { Toggle } from "@/components/ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"

import { FormSpecimen } from "./FormSpecimen"
import { Section, Specimen } from "./Section"

const BUTTON_VARIANTS = [
  "default",
  "secondary",
  "outline",
  "ghost",
  "destructive",
  "link",
] as const
const BADGE_VARIANTS = [
  "default",
  "secondary",
  "outline",
  "destructive",
  "ghost",
] as const

export function Controls() {
  const [slider, setSlider] = useState([40])
  return (
    <>
      <Section id="buttons" title="Buttons and badges">
        <Specimen label="Button variants">
          {BUTTON_VARIANTS.map((variant) => (
            <Button key={variant} variant={variant}>
              {variant}
            </Button>
          ))}
        </Specimen>
        <Specimen label="Sizes">
          <Button size="xs">xs</Button>
          <Button size="sm">sm</Button>
          <Button>default</Button>
          <Button size="lg">lg</Button>
          <Button size="icon" aria-label="Search">
            <SearchIcon />
          </Button>
        </Specimen>
        <Specimen label="States">
          <Button disabled>disabled</Button>
          <Button disabled>
            <Spinner data-icon="inline-start" />
            saving
          </Button>
          <Button variant="outline">
            <MailIcon data-icon="inline-start" />
            with icon
          </Button>
        </Specimen>
        <Specimen label="Badge variants">
          {BADGE_VARIANTS.map((variant) => (
            <Badge key={variant} variant={variant}>
              {variant}
            </Badge>
          ))}
        </Specimen>
      </Section>
      <Section id="forms" title="Form controls" className="items-start">
        <Specimen
          label="Input and textarea"
          className="w-72 flex-col items-stretch"
        >
          <Label htmlFor="g-input">Workspace name</Label>
          <Input id="g-input" placeholder="Rust systems" />
          <Input
            aria-label="Invalid input"
            aria-invalid
            defaultValue="Taken name"
          />
          <Input aria-label="Disabled input" disabled defaultValue="Disabled" />
          <Textarea
            aria-label="Description"
            placeholder="What is this workspace for?"
          />
        </Specimen>
        <FormSpecimen />
        <Specimen label="Input group" className="w-72 flex-col items-stretch">
          <InputGroup>
            <InputGroupAddon>
              <SearchIcon aria-hidden />
            </InputGroupAddon>
            <InputGroupInput
              aria-label="Search nodes"
              placeholder="Search nodes"
            />
          </InputGroup>
        </Specimen>
        <Specimen label="Select" className="w-56">
          <Select defaultValue="teacher">
            <SelectTrigger aria-label="Role" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="prober">Prober</SelectItem>
              <SelectItem value="teacher">Teacher</SelectItem>
              <SelectItem value="practice">Practice</SelectItem>
            </SelectContent>
          </Select>
        </Specimen>
        <Specimen
          label="Checkbox, radio, switch"
          className="flex-col items-start"
        >
          <div className="flex items-center gap-2">
            <Checkbox id="g-check" defaultChecked />
            <Label htmlFor="g-check">Confirm destructive actions</Label>
          </div>
          <RadioGroup defaultValue="comfortable" aria-label="Density">
            <div className="flex items-center gap-2">
              <RadioGroupItem value="comfortable" id="g-r1" />
              <Label htmlFor="g-r1">Comfortable</Label>
            </div>
            <div className="flex items-center gap-2">
              <RadioGroupItem value="compact" id="g-r2" />
              <Label htmlFor="g-r2">Compact</Label>
            </div>
          </RadioGroup>
          <div className="flex items-center gap-2">
            <Switch id="g-switch" defaultChecked />
            <Label htmlFor="g-switch">Reduce motion</Label>
          </div>
        </Specimen>
        <Specimen label={`Slider (${slider[0]})`} className="w-56">
          <Slider
            value={slider}
            onValueChange={setSlider}
            max={100}
            step={1}
            aria-label="Confidence"
          />
        </Specimen>
        <Specimen label="Toggle and toggle group">
          <Toggle aria-label="Bold">
            <BoldIcon />
          </Toggle>
          <ToggleGroup
            type="multiple"
            variant="outline"
            aria-label="Text style"
          >
            <ToggleGroupItem value="bold" aria-label="Bold">
              <BoldIcon />
            </ToggleGroupItem>
            <ToggleGroupItem value="italic" aria-label="Italic">
              <ItalicIcon />
            </ToggleGroupItem>
            <ToggleGroupItem value="underline" aria-label="Underline">
              <UnderlineIcon />
            </ToggleGroupItem>
          </ToggleGroup>
        </Specimen>
      </Section>
    </>
  )
}
