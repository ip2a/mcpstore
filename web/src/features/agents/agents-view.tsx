import { RefreshCwIcon } from "lucide-react"

import { DetailHeader } from "@/components/shared/detail-header"
import { EntityRow } from "@/components/shared/entity-row"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { type AgentItem, type ServiceEntry } from "@/lib/api"
import { getAgentId, getAgentServices } from "@/features/agents/model"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { toolKey } from "@/lib/tool-info"

export function AgentsView(props: {
  agents: AgentItem[]
  services: ServiceEntry[]
  loading: boolean
  busy: string | null
  onAssign: (agentId: string, serviceName: string) => void
  onOpenService: (serviceName: string) => void
  onRefresh: () => void
  onUnassign: (agentId: string, serviceName: string) => void
}) {
  const {
    activeAgentId,
    agentIds,
    agentServices,
    agentServicesError,
    agentServicesErrorMessage,
    agentTools,
    agentToolsError,
    agentToolsErrorMessage,
    assignTarget,
    loadAgentScope,
    loadingAgentServices,
    loadingAgentTools,
    selectedAgentId,
    setAssignTarget,
    setSelectedAgentId,
    setTypedAgentId,
    typedAgentId,
  } = useAgentScope({ agents: props.agents, busy: props.busy, services: props.services })

  return (
    <>
      <DetailHeader
        eyebrow="Agent 管理"
        title="Agent Workspace"
        actions={
          <Button variant="outline" onClick={props.onRefresh} disabled={props.loading}>
            <RefreshCwIcon data-icon="inline-start" />
            刷新
          </Button>
        }
      />

      <TwoPanePage variant="page">
        <div className="flex min-w-0 flex-col gap-4">
          <PanelCard>
            <SectionHeading title="Agent List" titleAs="h2" description={`${props.agents.length} items`} className="border-b-0 pb-0" />
            <div>
              {props.loading ? (
                <PageSkeleton />
              ) : props.agents.length ? (
                <div className="flex flex-col gap-2">
                  {props.agents.map((agent) => {
                    const agentId = getAgentId(agent)
                    const serviceNames = getAgentServices(agent)
                    return (
                      <SelectableRowButton
                        key={agentId || JSON.stringify(agent)}
                        disabled={!agentId}
                        meta={`${serviceNames.length} services · ${agentId === activeAgentId ? agentTools.length : "-"} tools`}
                        onClick={() => setSelectedAgentId(agentId || null)}
                        selected={agentId === activeAgentId}
                        title={agentId || "-"}
                        trailing={agentId === activeAgentId ? <Badge variant="outline">active</Badge> : null}
                      />
                    )
                  })}
                </div>
              ) : (
                <PageEmpty title="No agents" description="Agent records will appear after services are assigned." onRefresh={props.onRefresh} />
              )}
            </div>
          </PanelCard>

          <PanelCard>
            <SectionHeading title="Agent Scope" titleAs="h2" description={activeAgentId || "No agent selected"} className="border-b-0 pb-0" />
            <div>
              <FieldGroup>
                <Field>
                  <FieldLabel>Known Agent</FieldLabel>
                  <Select value={selectedAgentId || "none"} onValueChange={(value) => setSelectedAgentId(value === "none" ? null : value)}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="none">None</SelectItem>
                        {agentIds.map((agentId) => (
                          <SelectItem key={agentId} value={agentId}>
                            {agentId}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <FieldLabel htmlFor="agent-id">Agent ID</FieldLabel>
                  <Input id="agent-id" value={typedAgentId} onChange={(event) => setTypedAgentId(event.target.value)} placeholder="agent-a" />
                </Field>
                <Field>
                  <FieldLabel>Assign Service</FieldLabel>
                  <Select value={assignTarget || "none"} onValueChange={(value) => setAssignTarget(value === "none" ? "" : value)}>
                    <SelectTrigger>
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="none">None</SelectItem>
                        {props.services.map((service) => (
                          <SelectItem key={service.name} value={service.name}>
                            {service.name}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Button disabled={!activeAgentId || !assignTarget || Boolean(props.busy)} onClick={() => props.onAssign(activeAgentId, assignTarget)}>
                  Assign
                </Button>
              </FieldGroup>
            </div>
          </PanelCard>
        </div>

        <div className="flex min-w-0 flex-col gap-4">
          <PanelCard>
            <SectionHeading title="Agent Services" titleAs="h2" description={loadingAgentServices ? "Loading" : `${agentServices.length} items`} className="border-b-0 pb-0" />
            <div>
              {agentServicesError ? (
                <PageError title="Agent services failed to load" message={agentServicesErrorMessage} onRefresh={loadAgentScope} />
              ) : loadingAgentServices ? (
                <PageSkeleton />
              ) : agentServices.length ? (
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Service</TableHead>
                      <TableHead>Status</TableHead>
                      <TableHead className="text-right">Actions</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {agentServices.map((service) => (
                      <TableRow key={service.name}>
                        <TableCell className="font-medium">{service.name}</TableCell>
                        <TableCell>
                          <ServiceStatusBadge status={service.status} />
                        </TableCell>
                        <TableCell className="text-right">
                          <div className="flex justify-end gap-2">
                            <Button size="sm" variant="outline" onClick={() => props.onOpenService(service.name)}>
                              View
                            </Button>
                            <Button size="sm" variant="outline" onClick={() => props.onUnassign(activeAgentId, service.name)} disabled={!activeAgentId || Boolean(props.busy)}>
                              Unassign
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              ) : (
                <PageEmpty title="No services" description="No MCP services are available for this agent." onRefresh={loadAgentScope} />
              )}
            </div>
          </PanelCard>

          <PanelCard>
            <SectionHeading title="Agent Tools" titleAs="h2" description={loadingAgentTools ? "Loading" : `${agentTools.length} items`} className="border-b-0 pb-0" />
            <div>
              {agentToolsError ? (
                <PageError title="Agent tools failed to load" message={agentToolsErrorMessage} onRefresh={loadAgentScope} />
              ) : loadingAgentTools ? (
                <PageSkeleton />
              ) : agentTools.length ? (
                <div className="flex flex-col gap-3">
                  {agentTools.slice(0, 8).map((tool) => (
                    <EntityRow key={toolKey(tool)} actions={<Badge variant="outline">tool</Badge>}>
                      <div className="flex min-w-0 flex-col gap-1">
                        <p className="truncate text-sm font-medium">{tool.name}</p>
                        <p className="truncate text-sm text-muted-foreground">{tool.description || "No description"}</p>
                      </div>
                    </EntityRow>
                  ))}
                </div>
              ) : (
                <PageEmpty title="No tools" description="No tools are available for this agent." onRefresh={loadAgentScope} />
              )}
            </div>
          </PanelCard>
        </div>
      </TwoPanePage>
    </>
  )
}
