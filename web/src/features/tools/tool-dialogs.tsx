import { useEffect, useState, type FormEvent } from "react"
import { toast } from "sonner"

import { DialogForm, DialogFormFooter } from "@/components/shared/dialog-form"
import { JsonBlock } from "@/components/shared/json-block"
import {
  ToolAnnotationsSection,
  ToolMetaSection,
  ToolOutputSchemaSection,
} from "@/components/shared/tool-capability-sections"
import { ToolServiceDetailGrid } from "@/features/tools/tool-service-detail-grid"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Field, FieldLabel } from "@/components/ui/field"
import { InputGroup, InputGroupTextarea } from "@/components/ui/input-group"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, InstanceStatus, ToolInfo } from "@/lib/api"
import { getToolSchema } from "@/lib/tool-info"

export type ToolDialogState = {
  tool: ToolInfo
  service: ServiceInstance
  sourceLabel: string
  initialArgs?: Record<string, unknown>
  onRun: (args: Record<string, unknown>) => Promise<unknown>
} | null

export type ToolDetailState = {
  tool: ToolInfo
  service: ServiceInstance
  sourceLabel: string
  onRun?: (args: Record<string, unknown>) => Promise<unknown>
  statusReport?: InstanceStatus | null
} | null

export function RunToolDialog({ state, onOpenChange }: { state: ToolDialogState; onOpenChange: (open: boolean) => void }) {
  const { t } = useI18n()
  const [args, setArgs] = useState("{}")
  const [result, setResult] = useState<unknown>(null)
  const [running, setRunning] = useState(false)

  useEffect(() => {
    if (state) {
      setArgs(JSON.stringify(state.initialArgs || {}, null, 2))
      setResult(null)
    }
  }, [state])

  async function onRun(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    if (!state) return
    setRunning(true)
    try {
      const parsed = JSON.parse(args)
      if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") throw new Error(t("toolTestArgsMustBeObject"))
      setResult(await state.onRun(parsed))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("toolCallFailed"))
    } finally {
      setRunning(false)
    }
  }

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{state ? t("confirmExecute", { name: state.tool.name }) : null}</DialogTitle>
          <DialogDescription>
            {state ? t("confirmExecuteDescription", { source: state.sourceLabel }) : null}
          </DialogDescription>
        </DialogHeader>
        <DialogForm onSubmit={onRun}>
          <Field>
            <FieldLabel htmlFor="tool-args">{t("dynamicValue")}</FieldLabel>
            <div className="overflow-hidden rounded-lg border bg-muted/20">
              <InputGroup className="border-0 bg-transparent shadow-none">
                <InputGroupTextarea
                  id="tool-args"
                  value={args}
                  onChange={(event) => setArgs(event.target.value)}
                  rows={14}
                  className="min-h-[16rem] border-0 bg-transparent font-mono text-xs leading-relaxed shadow-none focus-visible:ring-0"
                />
              </InputGroup>
            </div>
          </Field>
          {result ? (
            <Field>
              <FieldLabel>{t("result")}</FieldLabel>
              <JsonBlock value={result} />
            </Field>
          ) : null}
          <DialogFormFooter onCancel={() => onOpenChange(false)} submitLabel={running ? t("executing") : t("execute")} submitting={running} />
        </DialogForm>
      </DialogContent>
    </Dialog>
  )
}

export function ToolDetailDialog({ state, onOpenChange, onRun }: { state: ToolDetailState; onOpenChange: (open: boolean) => void; onRun: (state: NonNullable<ToolDetailState>) => void }) {
  const { t } = useI18n()
  const schema = state ? getToolSchema(state.tool) : {}

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85dvh,720px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b px-4 py-3 sm:px-5">
          <DialogTitle className="font-mono">{state?.tool.name}</DialogTitle>
          <DialogDescription>{state?.sourceLabel}</DialogDescription>
        </DialogHeader>
        <div className="min-h-0 overflow-auto px-4 py-4 sm:px-5">
          {state?.tool.description ? <p className="mb-4 text-sm text-muted-foreground">{state.tool.description}</p> : null}
          {state?.service ? (
            <div className="flex flex-col gap-4">
              <ToolServiceDetailGrid
                tool={state.tool}
                service={state.service}
                statusReport={state.statusReport}
                sourceLabel={state.sourceLabel}
              />
              <ToolOutputSchemaSection tool={state.tool} />
              <ToolAnnotationsSection tool={state.tool} />
              <ToolMetaSection tool={state.tool} />
            </div>
          ) : state ? (
            <div className="flex flex-col gap-4">
              {state.tool.description ? (
                <p className="text-sm text-muted-foreground">{state.tool.description}</p>
              ) : null}
              <div>
                <p className="mb-2 text-sm font-medium">{t("paramSchema")}</p>
                <JsonBlock value={schema} />
              </div>
              <ToolOutputSchemaSection tool={state.tool} />
              <ToolAnnotationsSection tool={state.tool} />
              <ToolMetaSection tool={state.tool} />
            </div>
          ) : null}
        </div>
        <div className="border-t px-4 py-3 sm:px-5">
          <DialogFormFooter
            cancelLabel={t("close")}
            onCancel={() => onOpenChange(false)}
            submitButtonProps={{
              className: state?.onRun ? undefined : "hidden",
              type: "button",
              onClick: () => state && onRun(state),
            }}
            submitLabel={t("run")}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}
