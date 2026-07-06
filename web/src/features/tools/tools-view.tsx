import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { DetailHeader } from "@/components/shared/detail-header"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { ToolCard } from "@/components/shared/tool-card"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { callAgentTool, callStoreTool, listAgentTools, listTools, type AgentItem, type ServiceEntry, type ToolInfo } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"
import { getToolServiceName, toolKey } from "@/lib/tool-info"
import type { ToolDetailState, ToolDialogState } from "@/features/tools/tool-dialogs"

export function ToolsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  onRunTool: (state: ToolDialogState) => void
  onToolDetail: (state: ToolDetailState) => void
}) {
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const [scope, setScope] = useState("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const [serviceName, setServiceName] = useState("all")
  const [query, setQuery] = useState("")
  const serviceFilter = serviceName === "all" ? undefined : serviceName
  const toolsQuery = useQuery({
    enabled: false,
    queryKey: scope === "agent" && agentId ? queryKeys.agentTools(agentId, serviceFilter) : queryKeys.tools(serviceFilter),
    queryFn: () => (scope === "agent" && agentId ? listAgentTools(agentId, serviceFilter) : listTools(serviceFilter)),
  })
  const tools = toolsQuery.data || []
  const error = toolsQuery.error
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : "工具加载失败"
  const loading = toolsQuery.isFetching

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  async function loadTools() {
    try {
      const nextTools = await toolsQuery.refetch()
      if (nextTools.error) throw nextTools.error
    } catch (err) {
      const message = err instanceof Error ? err.message : "工具加载失败"
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadTools()
  }, [scope, agentId, serviceName])

  const visibleTools = tools.filter((tool) => {
    const text = `${tool.name} ${tool.description || ""} ${getToolServiceName(tool) || ""}`.toLowerCase()
    return text.includes(query.trim().toLowerCase())
  })

  function makeRunner(tool: ToolInfo): NonNullable<ToolDialogState> {
    const sourceLabel = scope === "agent" ? `Agent ${agentId}` : getToolServiceName(tool) || serviceName
    return {
      tool,
      sourceLabel,
      onRun: (args) => (scope === "agent" ? callAgentTool(agentId, tool.name, args) : callStoreTool(tool.name, args)),
    }
  }

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
        <section className="grid gap-4 lg:grid-cols-2">
          {visibleTools.map((tool) => {
            const runner = makeRunner(tool)
            return (
              <ToolCard
                key={toolKey(tool)}
                tool={tool}
                sourceLabel={runner.sourceLabel}
                onRun={() => props.onRunTool(runner)}
                onDetail={() => props.onToolDetail(runner)}
              />
            )
          })}
        </section>
      ) : (
        <PageEmpty title="No tools" description="No tools are available in the current scope." onRefresh={loadTools} />
      )}
    </>
  )
}

function getAgentId(agent: AgentItem) {
  return String(agent.agent_id || agent.id || "")
}
