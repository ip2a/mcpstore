import { ClipboardIcon, EyeIcon, RefreshCwIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import {
  ToolDetailDocBody,
  ToolDetailDocHeader,
  ToolPlaygroundAside,
} from "@/components/shared/tool-detail-playground"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { ToolsFilterDialog } from "@/features/tools/tools-filter-dialog"
import { useToolArgsForm } from "@/features/tools/use-tool-args-form"
import { useToolsRegistry } from "@/features/tools/use-tools-registry"
import { useI18n } from "@/lib/i18n-context"
import { type AgentItem, type ServiceInstance, type ToolInfo } from "@/lib/api"
import { serializeToolArgs } from "@/lib/tool-args"
import { getToolSchema, toolKey } from "@/lib/tool-info"
import type { ToolDetailState, ToolDialogState } from "@/features/tools/tool-dialogs"
import { cn } from "@/lib/utils"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  onRunTool: (state: NonNullable<ToolDialogState>) => void
  onToolDetail: (state: ToolDetailState) => void
  isToolRunning?: (instanceId: string, toolName: string) => boolean
}) {
  const { t } = useI18n()
  const {
    activeFilterCount,
    agentId,
    agentIds,
    error,
    errorMessage,
    loadTools,
    loading,
    makeRunner,
    query,
    scope,
    selectedTool,
    selectedToolKey,
    instanceId,
    scopeInstances,
    setAgentId,
    setQuery,
    setScope,
    setSelectedToolKey,
    setInstanceId,
    clearToolPolicy,
    setVisibilityFilter,
    visibilityFilter,
    visibleTools,
  } = useToolsRegistry({ agents: props.agents, services: props.services })

  const { values: toolArgs, setField: setToolArg, schema: toolArgsSchema } = useToolArgsForm(
    selectedTool?.tool ?? null,
  )
  const runningSelectedTool = Boolean(
    selectedTool && props.isToolRunning?.(selectedTool.instance.instance_id, selectedTool.tool.name),
  )

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("tool")}</p>
            <h2 className="mt-1 truncate text-lg font-semibold">{t("toolRegistryTitle")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("toolRegistryDescription", {
                count: visibleTools.length,
                scope: scope === "agent" ? t("agentScopeLabel", { agentId: agentId || "-" }) : t("storeScope"),
              })}
            </p>
          </div>
        </section>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3">
          <div className="flex min-w-0 items-center justify-between gap-2">
            <h2 className="text-sm font-medium">{t("toolList")}</h2>
            <ToolsFilterDialog
              compact
              activeFilterCount={activeFilterCount}
              agentId={agentId}
              agentIds={agentIds}
              instanceId={instanceId}
              onAgentIdChange={setAgentId}
              onClearPolicy={
                instanceId !== "all"
                  ? () => {
                      const instance = scopeInstances.find((item) => item.instance_id === instanceId)
                      if (instance) void clearToolPolicy(instance)
                    }
                  : undefined
              }
              onInstanceIdChange={setInstanceId}
              onQueryChange={setQuery}
              onScopeChange={setScope}
              onVisibilityFilterChange={setVisibilityFilter}
              query={query}
              scope={scope}
              scopeInstances={scopeInstances}
              visibilityFilter={visibilityFilter}
            />
          </div>
          {error ? (
            <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
          ) : loading && !visibleTools.length ? (
            <PageSkeleton />
          ) : visibleTools.length ? (
            <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
              {visibleTools.map(({ instance, tool }) => {
                const key = toolKey(instance.instance_id, tool)
                const itemSchema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
                const itemParamCount = Object.keys(itemSchema.properties || {}).length
                const scopeLabel = instance.scope.type === "store" ? t("store") : `${t("agent")} ${instance.scope.agent_id}`
                return (
                  <SelectableRowButton
                    key={key}
                    meta={`${instance.service_name} · ${scopeLabel} · ${t("paramCount", { count: itemParamCount })}`}
                    onClick={() => setSelectedToolKey(key)}
                    selected={key === selectedToolKey}
                    title={tool.name}
                    trailing={
                      itemSchema.required?.length ? (
                        <Badge variant="outline">{itemSchema.required.length}</Badge>
                      ) : null
                    }
                  />
                )
              })}
            </ScrollPane>
          ) : (
            <PageEmpty title={t("noTools")} description={t("noToolsScopeDescription")} onRefresh={loadTools} />
          )}
        </div>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        <ToolPreviewHeader
          loading={loading}
          selectedTool={selectedTool?.tool ?? null}
          runningTool={runningSelectedTool}
          onRun={
            selectedTool
              ? () =>
                  props.onRunTool({
                    ...makeRunner(selectedTool),
                    initialArgs: serializeToolArgs(toolArgs, toolArgsSchema),
                  })
              : undefined
          }
          onDetail={selectedTool ? () => props.onToolDetail(makeRunner(selectedTool)) : undefined}
          onCopy={selectedTool ? () => void copyTool(selectedTool.tool) : undefined}
          onRefresh={loadTools}
        />

        {error ? (
          <PageError title={t("toolsFailedToLoad")} message={errorMessage} onRefresh={loadTools} />
        ) : loading && !visibleTools.length ? (
          <PageSkeleton />
        ) : selectedTool ? (
          <div className="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(12rem,18rem)] grid-rows-1 gap-6 overflow-hidden">
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ToolDetailDocHeader tool={selectedTool.tool} />
              <ScrollPane className="min-h-0 flex-1">
                <ToolDetailDocBody
                  tool={selectedTool.tool}
                  toolArgs={toolArgs}
                  onToolArgChange={setToolArg}
                />
              </ScrollPane>
            </div>
            <ToolPlaygroundAside
              tool={selectedTool.tool}
              instanceId={selectedTool.instance.instance_id}
              toolArgs={toolArgs}
              toolArgsSchema={toolArgsSchema}
              running={runningSelectedTool}
              onRun={() =>
                props.onRunTool({
                  ...makeRunner(selectedTool),
                  initialArgs: serializeToolArgs(toolArgs, toolArgsSchema),
                })
              }
            />
          </div>
        ) : (
          <PageEmpty
            title={t("noToolSelected")}
            description={t("noToolSelectedDescription")}
            onRefresh={loadTools}
          />
        )}
      </PanelCard>
    </TwoPanePage>
  )
}

function ToolPreviewHeader({
  loading,
  onCopy,
  onDetail,
  onRefresh,
  onRun,
  runningTool,
  selectedTool,
}: {
  loading: boolean
  onCopy?: () => void
  onDetail?: () => void
  onRefresh: () => void
  onRun?: () => void
  runningTool?: boolean
  selectedTool: ToolInfo | null | undefined
}) {
  const { t } = useI18n()
  const title = selectedTool?.name || t("noToolSelected")
  const hideTitle = Boolean(selectedTool)

  return (
    <div className={cn("flex flex-wrap items-center gap-3 border-b pb-2", hideTitle ? "justify-end" : "justify-between")}>
      {!hideTitle ? (
        <div className="flex min-w-0 flex-col gap-1">
          <strong className="truncate font-mono text-sm font-medium" title={title}>
            {title}
          </strong>
        </div>
      ) : null}
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {onRun ? (
          <Button size="sm" onClick={onRun} disabled={runningTool}>
            {runningTool ? <Spinner data-icon="inline-start" /> : <WrenchIcon data-icon="inline-start" />}
            {runningTool ? t("executing") : t("run")}
          </Button>
        ) : null}
        {onDetail ? (
          <Button size="sm" variant="outline" onClick={onDetail}>
            <EyeIcon data-icon="inline-start" />
            {t("details")}
          </Button>
        ) : null}
        {onCopy ? (
          <Button size="sm" variant="outline" onClick={onCopy}>
            <ClipboardIcon data-icon="inline-start" />
            {t("copy")}
          </Button>
        ) : null}
        <Button size="sm" variant="outline" onClick={onRefresh} disabled={loading}>
          <RefreshCwIcon data-icon="inline-start" />
          {t("refresh")}
        </Button>
      </div>
    </div>
  )
}

async function copyTool(tool: ToolInfo) {
  await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
  toast.success("Tool copied")
}
