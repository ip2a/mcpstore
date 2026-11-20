import { DialogFormFooter } from "@/components/shared/dialog-form"
import { JsonBlock } from "@/components/shared/json-block"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { ToolResultView } from "@/components/shared/tool-result-view"
import {
  ToolAnnotationsSection,
  ToolMetaSection,
  ToolOutputSchemaSection,
} from "@/components/shared/tool-capability-sections"
import { ToolServiceDetailGrid } from "@/features/tools/tool-service-detail-grid"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ServiceState, ToolInfo } from "@/lib/api"
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
  statusReport?: ServiceState | null
} | null

export type ToolResultState = {
  tool: ToolInfo
  sourceLabel: string
  result: unknown
} | null

export function ToolResultDialog({
  state,
  onOpenChange,
}: {
  state: ToolResultState
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85dvh,720px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="border-b px-4 py-4 sm:px-5">
          <DialogTitle>{state ? t("result") : null}</DialogTitle>
        </DialogHeader>
        <ScrollPane className="min-h-0 flex-1 px-4 py-4 sm:px-5">
          {state ? <ToolResultView result={state.result} /> : null}
        </ScrollPane>
        <div className="border-t px-4 py-3 sm:px-5">
          <DialogFormFooter
            cancelLabel={t("close")}
            onCancel={() => onOpenChange(false)}
            submitButtonProps={{ className: "hidden" }}
            submitLabel={t("close")}
          />
        </div>
      </DialogContent>
    </Dialog>
  )
}

export function ToolDetailDialog({
  state,
  onOpenChange,
  onRun,
}: {
  state: ToolDetailState
  onOpenChange: (open: boolean) => void
  onRun: (state: NonNullable<ToolDetailState>) => void
}) {
  const { t } = useI18n()
  const schema = state ? getToolSchema(state.tool) : {}

  return (
    <Dialog open={Boolean(state)} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85dvh,720px)] flex-col gap-0 overflow-hidden p-0 sm:max-w-3xl">
        <DialogHeader className="border-b px-4 py-4 sm:px-5">
          <DialogTitle className="font-mono">{state?.tool.name}</DialogTitle>
        </DialogHeader>
        <div className="min-h-0 overflow-auto px-4 py-4 sm:px-5">
          {state?.tool.description ? (
            <p className="mb-4 text-sm text-muted-foreground">{state.tool.description}</p>
          ) : null}
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
