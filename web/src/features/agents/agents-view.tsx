import { useEffect, useMemo, useState } from "react"
import { LinkIcon, RefreshCwIcon, ServerIcon, UnlinkIcon, WrenchIcon } from "lucide-react"

import { CatalogTabTrigger, CatalogTabsList } from "@/components/shared/catalog-tabs-list"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import {
  toolDetailSectionAside,
  toolDetailSectionGrid,
  toolDetailSectionLabel,
} from "@/components/shared/tool-detail-section-layout"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import { type AgentItem, type ServiceInstance } from "@/lib/api"
import { getAgentId } from "@/features/agents/model"
import type { InstanceTool } from "@/features/tools/queries"
import { useAgentScope } from "@/features/agents/use-agent-scope"
import { useI18n } from "@/lib/i18n-context"
import { toolKey } from "@/lib/tool-info"
import { cn } from "@/lib/utils"

type AgentCatalogTab = "services" | "tools"

export function AgentsView(props: {
  agents: AgentItem[]
  services: ServiceInstance[]
  loading: boolean
  busy: string | null
  onDeclareScope: (agentId: string, serviceName: string) => void
  onOpenService: (instanceId: string) => void
  onRefresh: () => void
  onRemoveScope: (agentId: string, serviceName: string) => void
}) {
  const { t } = useI18n()
  const [activeTab, setActiveTab] = useState<AgentCatalogTab>("services")
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null)
  const [selectedToolKeyState, setSelectedToolKey] = useState<string | null>(null)
  const {
    activeAgentId,
    agentIds,
    agentServices,
    agentServicesError,
    agentServicesErrorMessage,
    agentTools,
    agentToolsError,
    agentToolsErrorMessage,
    loadAgentScope,
    loadingAgentServices,
    loadingAgentTools,
    selectedAgentId,
    scopeServiceName,
    serviceNames,
    setScopeServiceName,
    setSelectedAgentId,
    setTypedAgentId,
    typedAgentId,
  } = useAgentScope({ agents: props.agents, busy: props.busy, services: props.services })

  const selectedAgent = useMemo(() => {
    if (!activeAgentId) return null
    return props.agents.find((agent) => getAgentId(agent) === activeAgentId) || null
  }, [activeAgentId, props.agents])

  const selectedService = useMemo(() => {
    if (!agentServices.length) return null
    return agentServices.find((service) => service.instance_id === selectedInstanceId) || agentServices[0]
  }, [agentServices, selectedInstanceId])

  const selectedTool = useMemo(() => {
    if (!agentTools.length) return null
    return agentTools.find(({ instance, tool }) => toolKey(instance.instance_id, tool) === selectedToolKeyState) || agentTools[0]
  }, [agentTools, selectedToolKeyState])

  const loadingScope = loadingAgentServices || loadingAgentTools
  const scopeError = agentServicesError || agentToolsError
  const scopeErrorMessage = agentServicesError ? agentServicesErrorMessage : agentToolsErrorMessage

  useEffect(() => {
    if (!agentServices.length) {
      setSelectedInstanceId(null)
      return
    }
    if (!selectedInstanceId || !agentServices.some((service) => service.instance_id === selectedInstanceId)) {
      setSelectedInstanceId(agentServices[0].instance_id)
    }
  }, [agentServices, selectedInstanceId])

  useEffect(() => {
    if (!agentTools.length) {
      setSelectedToolKey(null)
      return
    }
    if (!selectedToolKeyState || !agentTools.some(({ instance, tool }) => toolKey(instance.instance_id, tool) === selectedToolKeyState)) {
      setSelectedToolKey(toolKey(agentTools[0].instance.instance_id, agentTools[0].tool))
    }
  }, [agentTools, selectedToolKeyState])

  return (
    <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
      <PanelCard className="@container flex h-full min-h-0 flex-col">
        <section className="flex flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">{t("agent")}</p>
            <h2 className="mt-1 truncate text-lg font-semibold">{t("agentWorkspaceTitle")}</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("agentWorkspaceDescription", { count: props.agents.length })}
            </p>
          </div>
        </section>

        <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden pt-3">
          <SectionHeading
            title={t("agentList")}
            titleAs="h2"
            description={t("items", { count: props.agents.length })}
            descriptionPlacement="inline"
            className="border-b-0 pb-0"
          />
          {props.loading ? (
            <PageSkeleton />
          ) : props.agents.length ? (
            <ScrollPane className="max-h-44 shrink-0" innerClassName="flex flex-col gap-2">
              {props.agents.map((agent) => {
                const agentId = getAgentId(agent)
                return (
                  <SelectableRowButton
                    key={agentId || JSON.stringify(agent)}
                    disabled={!agentId}
                    meta={t("agentServicesToolsCount", {
                      services: agent.instance_ids.length,
                      tools: agentId === activeAgentId ? agentTools.length : 0,
                    })}
                    onClick={() => setSelectedAgentId(agentId || null)}
                    selected={agentId === activeAgentId}
                    title={agentId || "-"}
                    trailing={agentId === activeAgentId ? <Badge variant="outline">{t("active")}</Badge> : null}
                  />
                )
              })}
            </ScrollPane>
          ) : (
            <PageEmpty
              title={t("noAgents")}
              description={t("noAgentsDescription")}
              onRefresh={props.onRefresh}
            />
          )}

          <Tabs
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as AgentCatalogTab)}
            className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
          >
            <CatalogTabsList>
              <CatalogTabTrigger value="services" label={t("services")}>
                <ServerIcon />
              </CatalogTabTrigger>
              <CatalogTabTrigger value="tools" label={t("tools")}>
                <WrenchIcon />
              </CatalogTabTrigger>
            </CatalogTabsList>

            <TabsContent value="services" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading
                title={t("serviceList")}
                titleAs="h2"
                description={loadingAgentServices ? t("loading") : t("items", { count: agentServices.length })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {!activeAgentId ? (
                <PageEmpty
                  title={t("noAgentSelected")}
                  description={t("noAgentSelectedServicesDescription")}
                />
              ) : agentServicesError ? (
                <PageError
                  title={t("agentServicesFailedToLoad")}
                  message={agentServicesErrorMessage}
                  onRefresh={loadAgentScope}
                />
              ) : loadingAgentServices ? (
                <PageSkeleton />
              ) : agentServices.length ? (
                <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                  {agentServices.map((service) => (
                    <SelectableRowButton
                      key={service.instance_id}
                      meta={service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`}
                      onClick={() => setSelectedInstanceId(service.instance_id)}
                      selected={service.instance_id === selectedService?.instance_id}
                      title={service.service_name}
                      trailing={<ServiceStatusBadge status={service.status} />}
                    />
                  ))}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noServices")}
                  description={t("noServicesDescription")}
                  onRefresh={loadAgentScope}
                />
              )}
            </TabsContent>

            <TabsContent value="tools" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading
                title={t("toolList")}
                titleAs="h2"
                description={loadingAgentTools ? t("loading") : t("items", { count: agentTools.length })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {!activeAgentId ? (
                <PageEmpty
                  title={t("noAgentSelected")}
                  description={t("noAgentSelectedToolsDescription")}
                />
              ) : agentToolsError ? (
                <PageError
                  title={t("agentToolsFailedToLoad")}
                  message={agentToolsErrorMessage}
                  onRefresh={loadAgentScope}
                />
              ) : loadingAgentTools ? (
                <PageSkeleton />
              ) : agentTools.length ? (
                <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                  {agentTools.map(({ instance, tool }) => {
                    const key = toolKey(instance.instance_id, tool)
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={`${instance.service_name} · ${tool.description || t("noDescription")}`}
                        onClick={() => setSelectedToolKey(key)}
                        selected={key === selectedToolKeyState}
                        title={tool.name}
                      />
                    )
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noTools")}
                  description={t("noToolsAgentDescription")}
                  onRefresh={loadAgentScope}
                />
              )}
            </TabsContent>
          </Tabs>
        </div>

        <section className="mt-3 shrink-0 border-t pt-4">
          <SectionHeading
            title={t("agentScope")}
            titleAs="h2"
            description={activeAgentId || t("noAgentSelected")}
            className="border-b-0 pb-3"
          />
          <FieldGroup>
            <Field>
              <FieldLabel>{t("knownAgent")}</FieldLabel>
              <Select
                value={selectedAgentId || "none"}
                onValueChange={(value) => setSelectedAgentId(value === "none" ? null : value)}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="none">{t("none")}</SelectItem>
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
              <FieldLabel htmlFor="agent-id">{t("agentId")}</FieldLabel>
              <Input
                id="agent-id"
                value={typedAgentId}
                onChange={(event) => setTypedAgentId(event.target.value)}
                placeholder={t("agentIdPlaceholder")}
              />
            </Field>
            <Field>
              <FieldLabel>{t("service")} · {t("scope")}</FieldLabel>
              <Select
                value={scopeServiceName || "none"}
                onValueChange={(value) => setScopeServiceName(value === "none" ? "" : value)}
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="none">{t("none")}</SelectItem>
                    {serviceNames.map((serviceName) => (
                      <SelectItem key={serviceName} value={serviceName}>
                        {serviceName}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Button
              disabled={!activeAgentId || !scopeServiceName || Boolean(props.busy)}
              onClick={() => props.onDeclareScope(activeAgentId, scopeServiceName)}
            >
              {t("add")}
            </Button>
          </FieldGroup>
        </section>
      </PanelCard>

      <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
        <AgentPreviewHeader
          activeTab={activeTab}
          activeAgentId={activeAgentId}
          loading={props.loading || loadingScope}
          selectedService={selectedService}
          selectedTool={selectedTool}
          serviceCount={agentServices.length}
          toolCount={agentTools.length}
          onRefresh={() => {
            void props.onRefresh()
            void loadAgentScope()
          }}
          onOpenService={selectedService ? () => props.onOpenService(selectedService.instance_id) : undefined}
          onRemoveScope={
            activeAgentId && selectedService
              ? () => props.onRemoveScope(activeAgentId, selectedService.service_name)
              : undefined
          }
          busy={Boolean(props.busy)}
        />

        {activeAgentId ? <AgentSummarySection agentId={activeAgentId} services={agentServices} /> : null}

        <MetricGrid columns="four">
          <MetricTile
            variant="compact"
            label={t("agent")}
            value={activeAgentId || "-"}
            title={activeAgentId || "-"}
            hint={selectedAgent ? t("inScope", { count: selectedAgent.instance_ids.length }) : t("selectOrTypeAgentId")}
          />
          <MetricTile
            variant="compact"
            label={t("services")}
            value={String(agentServices.length)}
            hint={loadingAgentServices ? t("loading") : t("inScope", { count: agentServices.length })}
          />
          <MetricTile
            variant="compact"
            label={t("tools")}
            value={String(agentTools.length)}
            hint={loadingAgentTools ? t("loading") : t("toolsAvailable", { count: agentTools.length })}
          />
          <MetricTile
            variant="compact"
            label={t("catalog")}
            value={String(props.services.length)}
            hint={scopeServiceName || t("none")}
          />
        </MetricGrid>

        <ScrollPane className="flex-1">
          {!activeAgentId ? (
            <PageEmpty
              title={t("noAgentSelected")}
              description={t("noAgentSelectedScopeDescription")}
            />
          ) : scopeError ? (
            <PageError
              title={t("agentScopeFailedToLoad")}
              message={scopeErrorMessage}
              onRefresh={loadAgentScope}
            />
          ) : activeTab === "services" ? (
            selectedService ? (
              <AgentServiceDetailPane service={selectedService} />
            ) : (
              <PageEmpty
                title={t("noServiceSelected")}
                description={t("noServicesDescription")}
                onRefresh={loadAgentScope}
              />
            )
          ) : selectedTool ? (
              <AgentToolDetailPane item={selectedTool} />
          ) : (
            <PageEmpty
              title={t("noToolSelected")}
              description={t("agentToolDetailsWillAppear")}
              onRefresh={loadAgentScope}
            />
          )}
        </ScrollPane>
      </PanelCard>
    </TwoPanePage>
  )
}

function AgentPreviewHeader({
  activeTab,
  activeAgentId,
  busy,
  loading,
  onOpenService,
  onRefresh,
  onRemoveScope,
  selectedService,
  selectedTool,
  serviceCount,
  toolCount,
}: {
  activeTab: AgentCatalogTab
  activeAgentId: string
  busy: boolean
  loading: boolean
  onOpenService?: () => void
  onRefresh: () => void
  onRemoveScope?: () => void
  selectedService: ServiceInstance | null
  selectedTool: InstanceTool | null
  serviceCount: number
  toolCount: number
}) {
  const { t } = useI18n()
  const title =
    activeTab === "services"
      ? selectedService?.service_name || (activeAgentId ? t("servicesAvailable", { count: serviceCount }) : t("noAgentSelected"))
      : selectedTool?.tool.name || (activeAgentId ? t("toolsAvailable", { count: toolCount }) : t("noAgentSelected"))

  const hideTitle = activeTab === "tools" && Boolean(selectedTool)

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
        {activeTab === "services" && onOpenService ? (
          <Button size="sm" variant="outline" onClick={onOpenService}>
            <LinkIcon data-icon="inline-start" />
            {t("view")}
          </Button>
        ) : null}
        {activeTab === "services" && onRemoveScope ? (
          <Button size="sm" variant="outline" onClick={onRemoveScope} disabled={busy}>
            <UnlinkIcon data-icon="inline-start" />
            {t("delete")} {t("scope")}
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

function AgentSummarySection({ agentId, services }: { agentId: string; services: ServiceInstance[] }) {
  const { t } = useI18n()

  return (
    <section className="border-b pb-4">
      <div className={toolDetailSectionGrid}>
        <div className={toolDetailSectionAside}>
          <h2 className={cn(toolDetailSectionLabel, "font-mono")} title={agentId}>
            {agentId}
          </h2>
        </div>
        <p className="break-words text-right text-sm text-muted-foreground">
          {services.length ? services.map((service) => service.service_name).join(" · ") : t("noServices")}
        </p>
      </div>
    </section>
  )
}

function AgentServiceDetailPane({ service }: { service: ServiceInstance }) {
  const { t } = useI18n()
  const endpoint = service.url || service.command || "-"
  const scope = service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("service")} titleAs="h2" className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("name")}</dt>
            <dd className="break-words font-mono">{service.service_name}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">Instance ID</dt>
            <dd className="break-all font-mono">{service.instance_id}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("scope")}</dt>
            <dd className="font-mono">{scope}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("status")}</dt>
            <dd>
              <ServiceStatusBadge status={service.status} />
            </dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("endpoint")}</dt>
            <dd className="font-mono break-all">{String(endpoint)}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("transport")}</dt>
            <dd className="font-mono">{service.transport}</dd>
          </div>
        </dl>
      </section>
    </div>
  )
}

function AgentToolDetailPane({ item }: { item: InstanceTool }) {
  const { t } = useI18n()
  const { instance, tool } = item
  const scope = instance.scope.type === "store" ? t("store") : `${t("agent")} ${instance.scope.agent_id}`

  return (
    <div className="@container flex min-w-0 flex-col">
      <section className="border-b pb-4">
        <div className={toolDetailSectionGrid}>
          <div className={toolDetailSectionAside}>
            <h2 className={cn(toolDetailSectionLabel, "font-mono")} title={tool.name}>
              {tool.name}
            </h2>
          </div>
          <ToolDescriptionBlock description={tool.description} showLabel={false} className="text-right" />
        </div>
      </section>
      <section className="grid gap-3 pt-4 text-sm @min-[32rem]:grid-cols-2">
        <div>
          <p className="text-muted-foreground">{t("service")}</p>
          <p className="break-words font-mono">{instance.service_name}</p>
        </div>
        <div>
          <p className="text-muted-foreground">{t("scope")}</p>
          <p className="font-mono">{scope}</p>
        </div>
        <div className="@min-[32rem]:col-span-2">
          <p className="text-muted-foreground">Instance ID</p>
          <p className="break-all font-mono">{instance.instance_id}</p>
        </div>
      </section>
    </div>
  )
}
