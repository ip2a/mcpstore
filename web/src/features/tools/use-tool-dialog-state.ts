import { useState } from "react"

import { type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { callInstanceTool, type ServiceInstance, type InstanceStatus, type ToolInfo } from "@/lib/api"

export function useToolDialogState() {
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)

  function openServiceToolRunner(service: ServiceInstance, tool: ToolInfo, initialArgs?: Record<string, unknown>) {
    setToolDialog({
      tool,
      service,
      sourceLabel: service.scope.type === "store"
        ? `${service.service_name} · store`
        : `${service.service_name} · agent ${service.scope.agent_id}`,
      initialArgs,
      onRun: (args) => callInstanceTool(service.instance_id, tool.name, args),
    })
  }

  function openServiceToolDetail(service: ServiceInstance, tool: ToolInfo, statusReport?: InstanceStatus | null) {
    setToolDetail({
      tool,
      sourceLabel: service.scope.type === "store"
        ? `${service.service_name} · store`
        : `${service.service_name} · agent ${service.scope.agent_id}`,
      service,
      statusReport,
      onRun: (args) => callInstanceTool(service.instance_id, tool.name, args),
    })
  }

  function openToolRunnerFromDetail(state: NonNullable<ToolDetailState>) {
    if (!state.onRun) return
    setToolDialog({ tool: state.tool, service: state.service, sourceLabel: state.sourceLabel, onRun: state.onRun })
  }

  function closeToolDetail(open: boolean) {
    if (!open) setToolDetail(null)
  }

  function closeToolDialog(open: boolean) {
    if (!open) setToolDialog(null)
  }

  return {
    closeToolDetail,
    closeToolDialog,
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    setToolDetail,
    setToolDialog,
    toolDetail,
    toolDialog,
  }
}
