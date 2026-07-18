import { useCallback, useState } from "react"
import { toast } from "sonner"

import { type ToolDetailState, type ToolDialogState, type ToolResultState } from "@/features/tools/tool-dialogs"
import { callInstanceTool, type ServiceInstance, type ServiceState, type ToolInfo } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

function buildSourceLabel(service: ServiceInstance) {
  return service.scope.type === "store"
    ? `${service.service_name} · store`
    : `${service.service_name} · agent ${service.scope.agent_id}`
}

function toolRunKey(instanceId: string, toolName: string) {
  return `${instanceId}:${toolName}`
}

export function useToolDialogState() {
  const { t } = useI18n()
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)
  const [toolResult, setToolResult] = useState<ToolResultState>(null)
  const [toolRunningKey, setToolRunningKey] = useState<string | null>(null)

  const runServiceTool = useCallback(async (
    service: ServiceInstance,
    tool: ToolInfo,
    args: Record<string, unknown> = {},
  ) => {
    const key = toolRunKey(service.instance_id, tool.name)
    setToolRunningKey(key)
    try {
      const result = await callInstanceTool(service.instance_id, tool.name, args)
      setToolResult({
        tool,
        sourceLabel: buildSourceLabel(service),
        result,
      })
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("toolCallFailed"))
    } finally {
      setToolRunningKey(null)
    }
  }, [t])

  function openServiceToolRunner(service: ServiceInstance, tool: ToolInfo, initialArgs?: Record<string, unknown>) {
    void runServiceTool(service, tool, initialArgs || {})
  }

  function openServiceToolDetail(service: ServiceInstance, tool: ToolInfo, statusReport?: ServiceState | null) {
    setToolDetail({
      tool,
      sourceLabel: buildSourceLabel(service),
      service,
      statusReport,
      onRun: (args) => callInstanceTool(service.instance_id, tool.name, args),
    })
  }

  function openToolRunnerFromDetail(state: NonNullable<ToolDetailState>) {
    if (!state.onRun) return
    void runServiceTool(state.service, state.tool, {})
  }

  function runToolFromDialogState(state: NonNullable<ToolDialogState>) {
    void runServiceTool(state.service, state.tool, state.initialArgs || {})
  }

  function isToolRunning(instanceId: string, toolName: string) {
    return toolRunningKey === toolRunKey(instanceId, toolName)
  }

  function closeToolDetail(open: boolean) {
    if (!open) setToolDetail(null)
  }

  function closeToolDialog(open: boolean) {
    if (!open) setToolDialog(null)
  }

  function closeToolResult(open: boolean) {
    if (!open) setToolResult(null)
  }

  return {
    closeToolDetail,
    closeToolDialog,
    closeToolResult,
    isToolRunning,
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    runServiceTool,
    runToolFromDialogState,
    setToolDetail,
    setToolDialog,
    toolDetail,
    toolDialog,
    toolResult,
    toolRunningKey,
  }
}
