import { useState } from "react"

import { type ToolDetailState, type ToolDialogState } from "@/features/tools/tool-dialogs"
import { callTool, type ServiceEntry, type ToolInfo } from "@/lib/api"

export function useToolDialogState() {
  const [toolDialog, setToolDialog] = useState<ToolDialogState>(null)
  const [toolDetail, setToolDetail] = useState<ToolDetailState>(null)

  function openServiceToolRunner(service: ServiceEntry, tool: ToolInfo) {
    setToolDialog({
      tool,
      sourceLabel: service.name,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  function openServiceToolDetail(service: ServiceEntry, tool: ToolInfo) {
    setToolDetail({
      tool,
      sourceLabel: service.name,
      onRun: (args) => callTool(service.name, tool.name, args),
    })
  }

  function openToolRunnerFromDetail(state: NonNullable<ToolDetailState>) {
    if (!state.onRun) return
    setToolDialog({ tool: state.tool, sourceLabel: state.sourceLabel, onRun: state.onRun })
  }

  return {
    openServiceToolDetail,
    openServiceToolRunner,
    openToolRunnerFromDetail,
    setToolDetail,
    setToolDialog,
    toolDetail,
    toolDialog,
  }
}
