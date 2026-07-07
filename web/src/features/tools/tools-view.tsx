import { RefreshCwIcon } from "lucide-react"

import { DetailHeader } from "@/components/shared/detail-header"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { ToolCard } from "@/components/shared/tool-card"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useToolsRegistry } from "@/features/tools/use-tools-registry"
import { type AgentItem, type ServiceEntry, type ToolInfo } from "@/lib/api"
import { getToolSchema, getToolServiceName, toolKey } from "@/lib/tool-info"
import type { ToolDetailState, ToolDialogState } from "@/features/tools/tool-dialogs"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  onRunTool: (state: ToolDialogState) => void
  onToolDetail: (state: ToolDetailState) => void
}) {
  const {
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
    serviceName,
    setAgentId,
    setQuery,
    setScope,
    setSelectedToolKey,
    setServiceName,
    visibleTools,
  } = useToolsRegistry({ agents: props.agents })

  return (
    <>
      <DetailHeader
        eyebrow="工具管理"
        title="Tool Registry"
        actions={
          <Button variant="outline" onClick={loadTools} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <PanelCard>
        <SectionHeading title="Filters" titleAs="h2" description={`${visibleTools.length} tools`} className="border-b-0 pb-0" />
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_180px_220px_220px]">
          <SearchBox placeholder="Search tools" value={query} onChange={setQuery} />
          <Select value={scope} onValueChange={setScope}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="store">Store</SelectItem>
                <SelectItem value="agent">Agent</SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
          <Select value={agentId || "none"} onValueChange={(value) => setAgentId(value === "none" ? "" : value)} disabled={scope !== "agent"}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="none">No agent</SelectItem>
                {agentIds.map((id) => (
                  <SelectItem key={id} value={id}>
                    {id}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <Select value={serviceName} onValueChange={setServiceName}>
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="all">All services</SelectItem>
                {props.services.map((service) => (
                  <SelectItem key={service.name} value={service.name}>
                    {service.name}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>
      </PanelCard>

      {error ? (
        <PageError title="Tools failed to load" message={errorMessage} onRefresh={loadTools} />
      ) : loading ? (
        <PageSkeleton />
      ) : visibleTools.length ? (
        <TwoPanePage variant="page" className="min-h-[520px] lg:grid-cols-[minmax(260px,0.38fr)_minmax(0,1fr)]">
          <PanelCard>
            <SectionHeading title="Tool List" titleAs="h2" description={`${visibleTools.length} tools`} className="border-b-0 pb-0" />
            <div className="flex min-h-0 flex-col gap-2 overflow-auto pr-1">
              {visibleTools.map((tool) => {
                const key = toolKey(tool)
                const schema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
                const paramCount = Object.keys(schema.properties || {}).length
                return (
                  <SelectableRowButton
                    key={key}
                    meta={`${getToolServiceName(tool) || "store"} · ${paramCount} params`}
                    onClick={() => setSelectedToolKey(key)}
                    selected={key === selectedToolKey}
                    title={tool.name}
                    trailing={schema.required?.length ? <Badge variant="outline">{schema.required.length}</Badge> : null}
                  />
                )
              })}
            </div>
          </PanelCard>

          {selectedTool ? (
            <ToolDetailPane
              tool={selectedTool}
              sourceLabel={makeRunner(selectedTool).sourceLabel}
              onDetail={() => props.onToolDetail(makeRunner(selectedTool))}
              onRun={() => props.onRunTool(makeRunner(selectedTool))}
            />
          ) : (
            <PageEmpty title="No tool selected" description="Select a tool to inspect its schema and actions." />
          )}
        </TwoPanePage>
      ) : (
        <PageEmpty title="No tools" description="No tools are available in the current scope." onRefresh={loadTools} />
      )}
    </>
  )
}

function ToolDetailPane({ tool, sourceLabel, onDetail, onRun }: { tool: ToolInfo; sourceLabel: string; onDetail: () => void; onRun: () => void }) {
  const schema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
  const params = Object.keys(schema.properties || {})

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <MetricGrid columns="three">
        <MetricTile label="Params" value={params.length} hint="schema fields" variant="compact" />
        <MetricTile label="Required" value={schema.required?.length || 0} hint="mandatory" variant="compact" />
        <MetricTile label="Source" value={sourceLabel} hint="call scope" variant="compact" />
      </MetricGrid>
      <ToolCard tool={tool} sourceLabel={sourceLabel} onDetail={onDetail} onRun={onRun} />
    </div>
  )
}
