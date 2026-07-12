import { useState } from "react"

import { type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { callTool, type ServiceEntry, type ServiceStatusReport, type ToolInfo } from "@/lib/api"

export function useToolDialogState() {
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)

  function openServiceToolRunner(service: ServiceEntry, tool: ToolInfo, initialArgs?: Record<string, unknown>) {
    setToolDialog({
      tool,
      sourceLabel: service.name,
      initialArgs,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  function openServiceToolDetail(service: ServiceEntry, tool: ToolInfo, statusReport?: ServiceStatusReport | null) {
    setToolDetail({
      tool,
      sourceLabel: service.name,
      service,
      statusReport,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  function openToolRunnerFromDetail(state: NonNullable<ToolDetailState>) {
    if (!state.onRun) return
    setToolDialog({ tool: state.tool, sourceLabel: state.sourceLabel, onRun: state.onRun })
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
