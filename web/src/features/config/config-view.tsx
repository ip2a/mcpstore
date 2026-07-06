import { useEffect, useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { RefreshCwIcon } from "lucide-react"
import { toast } from "sonner"

import { DetailHeader } from "@/components/shared/detail-header"
import { JsonBlock } from "@/components/shared/json-block"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { Button } from "@/components/ui/button"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { showAgentConfig, showConfig, type AgentItem } from "@/lib/api"
import { queryKeys } from "@/lib/query-keys"

export type ResetTarget = { scope: "store" } | { scope: "agent"; agentId: string }

export function ConfigView(props: { agents: AgentItem[]; resetTarget: ResetTarget | null; onResetTarget: (target: ResetTarget | null) => void }) {
  const agentIds = props.agents.map(getAgentId).filter(Boolean)
  const [activeTab, setActiveTab] = useState("store")
  const [agentId, setAgentId] = useState(agentIds[0] || "")
  const storeConfigQuery = useQuery({ enabled: false, queryKey: queryKeys.config, queryFn: showConfig })
  const agentConfigQuery = useQuery({ enabled: false, queryKey: queryKeys.agentConfig(agentId), queryFn: () => showAgentConfig(agentId) })
  const storeConfig = storeConfigQuery.data
  const agentConfig = agentId ? agentConfigQuery.data : null
  const error = storeConfigQuery.error || agentConfigQuery.error
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : "配置加载失败"
  const loading = storeConfigQuery.isFetching || agentConfigQuery.isFetching

  useEffect(() => {
    if (!agentId && agentIds[0]) setAgentId(agentIds[0])
  }, [agentId, agentIds])

  async function loadConfig() {
    try {
      const store = await storeConfigQuery.refetch()
      if (store.error) throw store.error
      if (!agentId) return
      const agent = await agentConfigQuery.refetch()
      if (agent.error) throw agent.error
    } catch (err) {
      const message = err instanceof Error ? err.message : "配置加载失败"
      toast.error(message)
    }
  }

  useEffect(() => {
    void loadConfig()
  }, [agentId, props.resetTarget])

  return (
    <>
      <DetailHeader
        eyebrow="配置管理"
        title="Configuration"
        actions={
          <Button variant="outline" onClick={loadConfig} disabled={loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="store">Store</TabsTrigger>
          <TabsTrigger value="agent">Agent</TabsTrigger>
        </TabsList>
        <TabsContent value="store">
          <PanelCard>
            <SectionHeading
              title="Store Config"
              titleAs="h2"
              className="border-b-0 pb-0"
              actions={<Button variant="outline" size="sm" onClick={() => props.onResetTarget({ scope: "store" })}>
                <RefreshCwIcon data-icon="inline-start" />
                Reset
              </Button>}
            />
            {error ? (
              <PageError title="Configuration failed to load" message={errorMessage} onRefresh={loadConfig} />
            ) : loading && !storeConfig ? (
              <PageSkeleton />
            ) : (
              <JsonBlock value={storeConfig || {}} />
            )}
          </PanelCard>
        </TabsContent>
        <TabsContent value="agent">
          <PanelCard>
            <SectionHeading
              title="Agent Config"
              titleAs="h2"
              description={agentId || "No agent selected"}
              className="border-b-0 pb-0"
              actions={<Button variant="outline" size="sm" disabled={!agentId} onClick={() => props.onResetTarget({ scope: "agent", agentId })}>
                <RefreshCwIcon data-icon="inline-start" />
                Reset
              </Button>}
            />
            <Select value={agentId || "none"} onValueChange={(value) => setAgentId(value === "none" ? "" : value)}>
              <SelectTrigger className="w-full md:w-80">
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
            {error ? (
              <PageError title="Agent config failed to load" message={errorMessage} onRefresh={loadConfig} />
            ) : loading && !agentConfig ? (
              <PageSkeleton />
            ) : (
              <JsonBlock value={agentConfig || {}} />
            )}
          </PanelCard>
        </TabsContent>
      </Tabs>
    </>
  )
}

function getAgentId(agent: AgentItem) {
  return String(agent.agent_id || agent.id || "")
}
