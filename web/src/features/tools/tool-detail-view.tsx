import { useState } from "react"
import { ClipboardIcon, PanelRightCloseIcon, PanelRightIcon, RefreshCwIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { WorkspaceBreadcrumb } from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import {
  ToolDetailDocBody,
  ToolDetailDocHeader,
  ToolPlaygroundAside,
} from "@/components/shared/tool-detail-playground"
import { toolDetailPlaygroundGrid } from "@/components/shared/tool-detail-section-layout"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { useInstanceToolsQueries } from "@/features/tools/queries"
import { useToolArgsForm } from "@/features/tools/use-tool-args-form"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolInfo } from "@/lib/api"
import { serializeToolArgs } from "@/lib/tool-args"
import type { ToolDialogState } from "@/features/tools/tool-dialogs"
import { callInstanceTool } from "@/lib/api"
import { cn } from "@/lib/utils"

export function ToolDetailView(props: {
  instanceId: string
  toolName: string
  services: ServiceInstance[]
  onBack: () => void
  onRunTool: (state: NonNullable<ToolDialogState>) => void
  isToolRunning?: (instanceId: string, toolName: string) => boolean
}) {
  const { t } = useI18n()
  const instance = props.services.find((service) => service.instance_id === props.instanceId)
  const toolQueries = useInstanceToolsQueries(instance ? [instance] : [], "available")
  const tools = toolQueries[0]?.data || []
  const tool = tools.find((item) => item.name === props.toolName) ?? null
  const loading = Boolean(instance) && toolQueries.some((query) => query.isFetching)
  const error = toolQueries.find((query) => query.error)?.error
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("toolsFailedToLoad")

  const { values: toolArgs, setField: setToolArg, schema: toolArgsSchema } = useToolArgsForm(tool)
  const running = Boolean(tool && props.isToolRunning?.(props.instanceId, tool.name))
  const [playgroundOpen, setPlaygroundOpen] = useState(true)

  async function loadTools() {
    const results = await Promise.all(toolQueries.map((query) => query.refetch()))
    const failed = results.find((result) => result.error)
    if (failed?.error) {
      toast.error(failed.error instanceof Error ? failed.error.message : String(failed.error))
    }
  }

  function makeRunner(item: ToolInfo, service: ServiceInstance): NonNullable<ToolDialogState> {
    const scopeLabel = service.scope.type === "store" ? "store" : `agent ${service.scope.agent_id}`
    return {
      tool: item,
      service,
      sourceLabel: `${service.service_name} · ${scopeLabel}`,
      onRun: (args) => callInstanceTool(service, item.name, args),
    }
  }

  const toolLabel = tool?.name || props.toolName
  const scopeLabel = instance
    ? instance.scope.type === "store"
      ? t("storeScope")
      : t("agentScopeLabel", { agentId: instance.scope.agent_id || "-" })
    : undefined
  const headerSubtitle = instance
    ? [instance.service_name, scopeLabel].filter(Boolean).join(" · ")
    : undefined

  if (!instance) {
    return (
      <>
        <WorkspaceBreadcrumb
          ariaLabel={t("tool")}
          parentLabel={t("toolRegistryTitle")}
          onParentClick={props.onBack}
          currentLabel={toolLabel}
        />
        <PanelCard className="h-full min-h-0 flex-1">
          <PageEmpty
            title={t("noToolSelected")}
            description={t("noToolSelectedDescription")}
            onRefresh={props.onBack}
          />
        </PanelCard>
      </>
    )
  }

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("tool")}
        parentLabel={t("toolRegistryTitle")}
        onParentClick={props.onBack}
        currentLabel={toolLabel}
        subtitle={headerSubtitle}
      />
      <PanelCard
        variant="plain"
        className="flex h-full min-h-0 flex-1 flex-col gap-4 overflow-hidden"
      >
        <div className="flex flex-wrap items-center justify-end gap-2 border-b pb-2">
          <div className="flex shrink-0 flex-wrap justify-end gap-2">
            {tool ? (
              <Button
                size="sm"
                onClick={() =>
                  props.onRunTool({
                    ...makeRunner(tool, instance),
                    initialArgs: serializeToolArgs(toolArgs, toolArgsSchema),
                  })
                }
                disabled={running}
              >
                {running ? (
                  <Spinner data-icon="inline-start" />
                ) : (
                  <WrenchIcon data-icon="inline-start" />
                )}
                {running ? t("executing") : t("run")}
              </Button>
            ) : null}
            {tool ? (
              <Button size="sm" variant="outline" onClick={() => void copyTool(tool)}>
                <ClipboardIcon data-icon="inline-start" />
                {t("copy")}
              </Button>
            ) : null}
            <Button size="sm" variant="outline" onClick={() => void loadTools()} disabled={loading}>
              <RefreshCwIcon data-icon="inline-start" />
              {t("refresh")}
            </Button>
            {tool ? (
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPlaygroundOpen((open) => !open)}
                aria-label={playgroundOpen ? t("collapse") : t("expand")}
                title={playgroundOpen ? t("collapse") : t("expand")}
              >
                {playgroundOpen ? <PanelRightCloseIcon /> : <PanelRightIcon />}
              </Button>
            ) : null}
          </div>
        </div>

        {error ? (
          <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
        ) : loading && !tool ? (
          <PageSkeleton />
        ) : tool ? (
          <div
            className={cn(
              toolDetailPlaygroundGrid,
              !playgroundOpen && "@min-[42rem]:grid-cols-1",
            )}
          >
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ScrollPane className="min-h-0 flex-1">
                <ToolDetailDocHeader tool={tool} serviceState={instance.state} />
                <ToolDetailDocBody
                  tool={tool}
                  toolArgs={toolArgs}
                  onToolArgChange={setToolArg}
                  serviceName={instance.service_name}
                  scope={instance.scope}
                />
              </ScrollPane>
            </div>
            {playgroundOpen ? (
              <ToolPlaygroundAside
                tool={tool}
                instanceId={instance.instance_id}
                toolArgs={toolArgs}
                toolArgsSchema={toolArgsSchema}
                running={running}
                onRun={() =>
                  props.onRunTool({
                    ...makeRunner(tool, instance),
                    initialArgs: serializeToolArgs(toolArgs, toolArgsSchema),
                  })
                }
              />
            ) : null}
          </div>
        ) : (
          <PageEmpty
            title={t("noTools")}
            description={t("noToolsScopeDescription")}
            onRefresh={loadTools}
          />
        )}
      </PanelCard>
    </>
  )
}

async function copyTool(tool: ToolInfo) {
  await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
  toast.success("Tool copied")
}
