import { useEffect, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { JsonBlock } from "@/components/shared/json-block"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupTextarea } from "@/components/ui/input-group"
import type { ToolInfo } from "@/lib/api"
import { getToolSchema } from "@/lib/tool-info"

export type ToolDialogState = {
  tool: ToolInfo
  sourceLabel: string
  onRun: (args: Record<string, unknown>) => Promise<unknown>
} | null

export type ToolDetailState = {
  tool: ToolInfo
  sourceLabel: string
  onRun?: (args: Record<string, unknown>) => Promise<unknown>
} | null

export function RunToolDialog({ state, onOpenChange }: { state: ToolDialogState; onOpenChange: (open: boolean) => void }) {
  const [args, setArgs] = useState("{}")
  const [result, setResult] = useState<unknown>(null)
  const [running, setRunning] = useState(false)

  useEffect(() => {
    if (state) {
      setArgs("{}")
      setResult(null)
    }
  }, [state])

  async function onRun(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!state) return
    setRunning(true)
    try {
      const parsed = JSON.parse(args)
      if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") throw new Error("Args must be a JSON object")
      setResult(await state.onRun(parsed))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Tool call failed")
    } finally {
      setRunning(false)
    }
  }

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Run tool: {state?.tool.name}</DialogTitle>
          <DialogDescription>{state?.sourceLabel}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onRun}>
          <Field>
            <FieldLabel htmlFor="tool-args">Args JSON</FieldLabel>
            <InputGroup>
              <InputGroupTextarea id="tool-args" value={args} onChange={(event) => setArgs(event.target.value)} rows={6} />
            </InputGroup>
          </Field>
          {result ? <JsonBlock value={result} /> : null}
          <DialogFormFooter cancelLabel="Close" onCancel={() => onOpenChange(false)} submitLabel={running ? "Running" : "Run"} submitting={running} />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}

export function ToolDetailDialog({ state, onOpenChange, onRun }: { state: ToolDetailState; onOpenChange: (open: boolean) => void; onRun: (state: NonNullable<ToolDetailState>) => void }) {
  const schema = state ? getToolSchema(state.tool) : {}

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (state?.onRun) onRun(state)
  }

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{state?.tool.name}</DialogTitle>
          <DialogDescription>{state?.sourceLabel}</DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onSubmit}>
          {state?.tool.description ? (
            <Field>
              <FieldLabel>Description</FieldLabel>
              <p className="text-sm text-muted-foreground">{state.tool.description}</p>
            </Field>
          ) : null}
          <Field>
            <FieldLabel>Param Schema</FieldLabel>
            <JsonBlock value={schema} />
          </Field>
          <Field>
            <FieldLabel>Raw Tool</FieldLabel>
            <JsonBlock value={state?.tool || {}} />
          </Field>
          <DialogFormFooter
            cancelLabel="Close"
            onCancel={() => onOpenChange(false)}
            submitButtonProps={{ className: state?.onRun ? undefined : "hidden" }}
            submitLabel="Run"
          />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}
