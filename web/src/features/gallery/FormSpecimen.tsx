import { zodResolver } from "@hookform/resolvers/zod"
import { CheckIcon } from "lucide-react"
import { useState } from "react"
import { useForm } from "react-hook-form"
import { z } from "zod"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"

import { Specimen } from "./Section"

const schema = z.object({
  name: z
    .string()
    .trim()
    .min(2, "Use at least 2 characters")
    .max(40, "Use at most 40 characters"),
  description: z.string().max(200, "Use at most 200 characters").optional(),
})
type Values = z.infer<typeof schema>

/** The form pattern the workspace and settings screens use: RHF + zod, inline errors, pending submit. */
export function FormSpecimen() {
  const [saved, setSaved] = useState<string | null>(null)
  const {
    register,
    handleSubmit,
    reset,
    formState: { errors, isSubmitting },
  } = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", description: "" },
  })

  const onSubmit = async (values: Values) => {
    await new Promise((resolve) => setTimeout(resolve, 250))
    setSaved(values.name)
    reset()
  }

  return (
    <Specimen
      label="Form (react-hook-form + zod)"
      className="w-72 flex-col items-stretch"
    >
      <form
        noValidate
        onSubmit={(event) => {
          event.preventDefault()
          void handleSubmit(onSubmit)(event)
        }}
        className="flex flex-col gap-2"
        data-testid="gallery-form"
      >
        <Label htmlFor="g-form-name">Name</Label>
        <Input
          id="g-form-name"
          aria-invalid={errors.name ? true : undefined}
          aria-describedby={errors.name ? "g-form-name-error" : undefined}
          {...register("name")}
        />
        {errors.name ? (
          <p
            id="g-form-name-error"
            role="alert"
            className="text-[12px] text-destructive"
          >
            {errors.name.message}
          </p>
        ) : null}
        <Label htmlFor="g-form-desc">Description</Label>
        <Textarea id="g-form-desc" {...register("description")} />
        {errors.description ? (
          <p role="alert" className="text-[12px] text-destructive">
            {errors.description.message}
          </p>
        ) : null}
        <Button
          type="submit"
          disabled={isSubmitting}
          className="self-start"
          data-testid="gallery-form-submit"
        >
          {isSubmitting ? <Spinner data-icon="inline-start" /> : null}
          Save workspace
        </Button>
        {saved ? (
          <p
            role="status"
            className="flex items-center gap-1 text-[12px] text-success"
            data-testid="gallery-form-saved"
          >
            <CheckIcon aria-hidden className="size-3.5" />
            Saved {saved}
          </p>
        ) : null}
      </form>
    </Specimen>
  )
}
