import { useEffect, useMemo, useState } from "react"
import { ClipboardIcon, EyeIcon, FileIcon, LayoutTemplateIcon, MessageSquareIcon, RefreshCwIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { JsonBlock } from "@/components/shared/json-block"
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid"
import { PageEmpty, PageError } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SearchBox } from "@/components/shared/search-box"
import { SectionHeading } from "@/components/shared/section-heading"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { CatalogTabTrigger, CatalogTabsList } from "@/components/shared/catalog-tabs-list"
import { ToolDetailDocBody, ToolDetailDocHeader, ToolPlaygroundAside } from "@/components/shared/tool-detail-playground"
import { TwoPanePage } from "@/components/shared/two-pane-page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tabs, TabsContent } from "@/components/ui/tabs"
import {
  useServiceDetailQuery,
  useServicePromptsQuery,
  useServiceResourceTemplatesQuery,
  useServiceResourcesQuery,
  useServiceStatusQuery,
} from "@/features/services/queries"
import { ServiceAuthPanel } from "@/features/services/service-auth-panel"
import { ServiceStatusActionsDialog } from "@/features/services/service-status-actions-dialog"
import { useToolArgsForm } from "@/features/tools/use-tool-args-form"
import { serializeToolArgs, type ToolSchema } from "@/lib/tool-args"
import { getToolSchema, toolKey } from "@/lib/tool-info"
import {
  getResourceTemplateAnnotations,
  getResourceTemplateMeta,
  hasResourceTemplateAnnotations,
  hasResourceTemplateMeta,
  resourceTemplateKey,
  resourceTemplateMimeType,
  resourceTemplateUri,
} from "@/lib/resource-template-info"
import {
  readInstanceResource,
  type PromptInfo,
  type ResourceInfo,
  type ResourceTemplateInfo,
  type ServiceInstance,
  type InstanceStatus,
  type ToolInfo,
} from "@/lib/api"
import { formatServiceLaunchLine } from "@/lib/service-info"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

type CatalogTab = "tools" | "resources" | "templates" | "prompts"
type RightPaneView = "service" | "catalog"

function resourceKey(resource: ResourceInfo) {
  return resource.uri
}

function promptKey(prompt: PromptInfo) {
  return prompt.name
}

function resourceMimeType(resource: ResourceInfo) {
  return String(resource.mimeType || "").trim()
}

function promptArgCount(prompt: PromptInfo) {
  const args = prompt.arguments
  if (!args) return 0
  if (Array.isArray(args)) return args.length
  if (typeof args === "object" && args !== null && Array.isArray((args as { properties?: unknown[] }).properties)) {
    return (args as { properties: unknown[] }).properties.length
  }
  return 0
}

export function ServiceDetailView(props: {
  service: ServiceInstance
  busy: string | null
  refreshToken?: number
  onBack: () => void
  onRunTool: (tool: ToolInfo, args: Record<string, unknown>) => void
  onToolDetail: (tool: ToolInfo, service: ServiceInstance, statusReport?: InstanceStatus | null) => void
  onConnect: () => void
  onDisconnect: () => void
  onRestart: () => void
  onDelete: () => void
}) {
  const { t } = useI18n()
  const [detailError, setDetailError] = useState<string | null>(null)
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("service")
  const [activeTab, setActiveTab] = useState<CatalogTab>("tools")
  const [selectedToolKey, setSelectedToolKey] = useState<string | null>(null)
  const [selectedResourceKey, setSelectedResourceKey] = useState<string | null>(null)
  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string | null>(null)
  const [selectedPromptKey, setSelectedPromptKey] = useState<string | null>(null)
  const [statusDialogOpen, setStatusDialogOpen] = useState(false)
  const [toolSearchQuery, setToolSearchQuery] = useState("")
  const detailQuery = useServiceDetailQuery(props.service.instance_id)
  const statusQuery = useServiceStatusQuery(props.service.instance_id)
  const resourcesQuery = useServiceResourcesQuery(props.service.instance_id)
  const resourceTemplatesQuery = useServiceResourceTemplatesQuery(props.service.instance_id)
  const promptsQuery = useServicePromptsQuery(props.service.instance_id)
  const detail = detailQuery.data
  const statusReport = statusQuery.data
  const error = detailQuery.error || detailError
  const errorMessage = error instanceof Error ? error.message : error ? String(error) : t("serviceDetailLoadFailed")
  const loading =
    detailQuery.isFetching ||
    statusQuery.isFetching ||
    resourcesQuery.isFetching ||
    resourceTemplatesQuery.isFetching ||
    promptsQuery.isFetching
  const service = detail || props.service
  const endpoint = service.url || service.command || "-"
  const tools = service.tools || []
  const filteredTools = useMemo(() => {
    const query = toolSearchQuery.trim().toLowerCase()
    if (!query) return tools
    return tools.filter((tool) => `${tool.name} ${tool.description || ""}`.toLowerCase().includes(query))
  }, [toolSearchQuery, tools])
  const resources = resourcesQuery.data || []
  const resourceTemplates = resourceTemplatesQuery.data || []
  const prompts = promptsQuery.data || []
  const serviceConfig = service.effective_config
  const transport = service.transport
  const launchLine = formatServiceLaunchLine(service)
  const description = String(serviceConfig?.description || "").trim()
  const configArgs = Array.isArray(serviceConfig?.args) ? serviceConfig.args.map(String).join(" ") : null
  const availableToolCount = statusReport?.tools?.filter((item) => item.status === "available").length
  const selectedTool = useMemo(() => {
    if (!tools.length) return null
    return tools.find((tool) => toolKey(service.instance_id, tool) === selectedToolKey) || tools[0]
  }, [selectedToolKey, service.instance_id, tools])
  const selectedResource = useMemo(() => {
    if (!resources.length) return null
    return resources.find((resource) => resourceKey(resource) === selectedResourceKey) || resources[0]
  }, [resources, selectedResourceKey])
  const selectedTemplate = useMemo(() => {
    if (!resourceTemplates.length) return null
    return resourceTemplates.find((template) => resourceTemplateKey(template) === selectedTemplateKey) || resourceTemplates[0]
  }, [resourceTemplates, selectedTemplateKey])
  const selectedPrompt = useMemo(() => {
    if (!prompts.length) return null
    return prompts.find((prompt) => promptKey(prompt) === selectedPromptKey) || prompts[0]
  }, [prompts, selectedPromptKey])
  const { values: toolArgs, setField: setToolArg, schema: toolArgsSchema } = useToolArgsForm(selectedTool)

  useEffect(() => {
    if (!tools.length) {
      setSelectedToolKey(null)
      return
    }
    if (!selectedToolKey || !tools.some((tool) => toolKey(service.instance_id, tool) === selectedToolKey)) {
      setSelectedToolKey(toolKey(service.instance_id, tools[0]))
    }
  }, [selectedToolKey, service.instance_id, tools])

  useEffect(() => {
    if (!resources.length) {
      setSelectedResourceKey(null)
      return
    }
    if (!selectedResourceKey || !resources.some((resource) => resourceKey(resource) === selectedResourceKey)) {
      setSelectedResourceKey(resourceKey(resources[0]))
    }
  }, [resources, selectedResourceKey])

  useEffect(() => {
    if (!resourceTemplates.length) {
      setSelectedTemplateKey(null)
      return
    }
    if (!selectedTemplateKey || !resourceTemplates.some((template) => resourceTemplateKey(template) === selectedTemplateKey)) {
      setSelectedTemplateKey(resourceTemplateKey(resourceTemplates[0]))
    }
  }, [resourceTemplates, selectedTemplateKey])

  useEffect(() => {
    if (!prompts.length) {
      setSelectedPromptKey(null)
      return
    }
    if (!selectedPromptKey || !prompts.some((prompt) => promptKey(prompt) === selectedPromptKey)) {
      setSelectedPromptKey(promptKey(prompts[0]))
    }
  }, [prompts, selectedPromptKey])

  async function loadDetail() {
    try {
      setDetailError(null)
      const [nextDetail] = await Promise.all([
        detailQuery.refetch(),
        statusQuery.refetch(),
        resourcesQuery.refetch(),
        resourceTemplatesQuery.refetch(),
        promptsQuery.refetch(),
      ])
      if (nextDetail.error) throw nextDetail.error
    } catch (err) {
      const message = err instanceof Error ? err.message : t("serviceDetailLoadFailed")
      if (!detailQuery.error) setDetailError(message)
      toast.error(message)
    }
  }

  useEffect(() => {
    setRightPaneView("service")
    setToolSearchQuery("")
  }, [props.service.instance_id])

  useEffect(() => {
    void loadDetail()
  }, [props.refreshToken, props.service.instance_id])

  return (
    <>
      <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
        <PanelCard className="@container h-full min-h-0">
          <section className="flex flex-col gap-3 border-b pb-4">
            <div className="min-w-0">
              <p className="font-mono text-xs uppercase text-muted-foreground">{t("service")}</p>
              <button
                type="button"
                onClick={() => setRightPaneView("service")}
                className={cn(
                  "mt-1 block max-w-full cursor-pointer truncate border-0 bg-transparent p-0 text-left text-lg font-semibold underline-offset-4 outline-none transition-opacity",
                  "hover:underline active:opacity-70",
                )}
                title={service.service_name}
              >
                {service.service_name}
              </button>
              <p className="mt-1 truncate font-mono text-xs text-muted-foreground" title={launchLine}>
                {launchLine}
              </p>
              {description ? (
                <p className="mt-2 line-clamp-2 text-sm text-muted-foreground" title={description}>
                  {description}
                </p>
              ) : null}
            </div>
          </section>

          <Tabs
            value={activeTab}
            onValueChange={(value) => {
              setActiveTab(value as CatalogTab)
              setRightPaneView("catalog")
            }}
            className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
          >
            <CatalogTabsList>
              <CatalogTabTrigger value="tools" label={t("tools")}>
                <WrenchIcon />
              </CatalogTabTrigger>
              <CatalogTabTrigger value="resources" label={t("resources")}>
                <FileIcon />
              </CatalogTabTrigger>
              <CatalogTabTrigger value="templates" label={t("templates")}>
                <LayoutTemplateIcon />
              </CatalogTabTrigger>
              <CatalogTabTrigger value="prompts" label={t("prompts")}>
                <MessageSquareIcon />
              </CatalogTabTrigger>
            </CatalogTabsList>

            <TabsContent value="tools" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading title={t("toolList")} titleAs="h2" description={t("itemsCount", { count: filteredTools.length })} descriptionPlacement="inline" className="border-b-0 pb-0" />
              {tools.length ? (
                filteredTools.length ? (
                  <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                    {filteredTools.map((tool) => {
                      const key = toolKey(service.instance_id, tool)
                      const schema = getToolSchema(tool) as { properties?: Record<string, unknown>; required?: string[] }
                      const paramCount = Object.keys(schema.properties || {}).length
                      return (
                        <SelectableRowButton
                          key={key}
                          meta={t("paramCount", { count: paramCount })}
                          onClick={() => {
                            setSelectedToolKey(key)
                            setRightPaneView("catalog")
                          }}
                          selected={rightPaneView === "catalog" && key === toolKey(service.instance_id, selectedTool || tool)}
                          title={tool.name}
                          trailing={schema.required?.length ? <Badge variant="outline">{schema.required.length}</Badge> : null}
                        />
                      )
                    })}
                  </ScrollPane>
                ) : (
                  <PageEmpty title={t("noMatchingTools")} description={t("noMatchingToolsDescription")} onRefresh={loadDetail} />
                )
              ) : (
                <PageEmpty title={t("noToolsFound")} description={t("noToolsFoundDescription")} onRefresh={loadDetail} />
              )}
            </TabsContent>

            <TabsContent value="resources" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading title={t("resourceList")} titleAs="h2" description={t("itemsCount", { count: resources.length })} descriptionPlacement="inline" className="border-b-0 pb-0" />
              {resources.length ? (
                <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                  {resources.map((resource) => {
                    const key = resourceKey(resource)
                    const mimeType = resourceMimeType(resource)
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={mimeType || resource.uri}
                        onClick={() => {
                          setSelectedResourceKey(key)
                          setRightPaneView("catalog")
                        }}
                        selected={rightPaneView === "catalog" && key === resourceKey(selectedResource || resource)}
                        title={resource.title || resource.name || resource.uri}
                      />
                    )
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty title={t("noResourcesFound")} description={t("noResourcesFoundDescription")} onRefresh={loadDetail} />
              )}
            </TabsContent>

            <TabsContent value="templates" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading title={t("templateList")} titleAs="h2" description={t("itemsCount", { count: resourceTemplates.length })} descriptionPlacement="inline" className="border-b-0 pb-0" />
              {resourceTemplates.length ? (
                <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                  {resourceTemplates.map((template) => {
                    const key = resourceTemplateKey(template)
                    const mimeType = resourceTemplateMimeType(template)
                    const uriTemplate = resourceTemplateUri(template)
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={mimeType || uriTemplate}
                        onClick={() => {
                          setSelectedTemplateKey(key)
                          setRightPaneView("catalog")
                        }}
                        selected={rightPaneView === "catalog" && key === resourceTemplateKey(selectedTemplate || template)}
                        title={template.title || template.name || uriTemplate}
                      />
                    )
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty title={t("noTemplatesFound")} description={t("noTemplatesFoundDescription")} onRefresh={loadDetail} />
              )}
            </TabsContent>

            <TabsContent value="prompts" className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
              <SectionHeading title={t("promptList")} titleAs="h2" description={t("itemsCount", { count: prompts.length })} descriptionPlacement="inline" className="border-b-0 pb-0" />
              {prompts.length ? (
                <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                  {prompts.map((prompt) => {
                    const key = promptKey(prompt)
                    const argCount = promptArgCount(prompt)
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={prompt.description || (argCount ? t("paramCount", { count: argCount }) : t("noArgs"))}
                        onClick={() => {
                          setSelectedPromptKey(key)
                          setRightPaneView("catalog")
                        }}
                        selected={rightPaneView === "catalog" && key === promptKey(selectedPrompt || prompt)}
                        title={prompt.title || prompt.name}
                      />
                    )
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty title={t("noPromptsFound")} description={t("noPromptsFoundDescription")} onRefresh={loadDetail} />
              )}
            </TabsContent>
          </Tabs>
        </PanelCard>

        <PanelCard variant="plain" className="flex h-full min-h-0 flex-col gap-4 overflow-hidden">
          <ServicePreviewHeader
            rightPaneView={rightPaneView}
            activeTab={activeTab}
            loading={loading}
            service={service}
            selectedTool={selectedTool}
            selectedResource={selectedResource}
            selectedTemplate={selectedTemplate}
            selectedPrompt={selectedPrompt}
            toolCount={tools.length}
            resourceCount={resources.length}
            templateCount={resourceTemplates.length}
            promptCount={prompts.length}
            instanceId={service.instance_id}
            onRefresh={loadDetail}
            onRun={
              rightPaneView === "catalog" && activeTab === "tools" && selectedTool
                ? () => props.onRunTool(selectedTool, serializeToolArgs(toolArgs, toolArgsSchema))
                : undefined
            }
            onDetail={
              rightPaneView === "catalog" && activeTab === "tools" && selectedTool
                ? () => props.onToolDetail(selectedTool, service, statusReport)
                : undefined
            }
            toolSearchQuery={toolSearchQuery}
            onToolSearchQueryChange={setToolSearchQuery}
          />
          {rightPaneView === "service" ? (
            <MetricGrid columns="four">
              <MetricTile
                variant="compact"
                label={t("name")}
                value={service.service_name}
                title={service.service_name}
                hint={service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`}
              />
              <MetricTile
                variant="compact"
                label={t("endpoint")}
                value={String(endpoint)}
                title={String(endpoint)}
                hint={configArgs ? `${transport} · ${configArgs}` : t("transportSuffix", { transport })}
              />
              <MetricTile
                variant="compact"
                label={t("status")}
                value={String(service.status || t("unknown"))}
                title={String(service.status || t("unknown"))}
                hint={statusReport?.health_status || t("clickToManage")}
                active={statusDialogOpen}
                onClick={() => setStatusDialogOpen(true)}
              />
              <MetricTile
                variant="compact"
                label={t("catalog")}
                value={String(tools.length + resources.length + resourceTemplates.length + prompts.length)}
                hint={t("catalogSummary", {
                  tools: tools.length,
                  resources: resources.length,
                  templates: resourceTemplates.length,
                  prompts: prompts.length,
                })}
              />
            </MetricGrid>
          ) : null}
          {error ? (
            <ScrollPane className="flex-1">
              <PageError title={t("serviceDetailLoadFailed")} message={errorMessage} onRefresh={loadDetail} />
            </ScrollPane>
          ) : rightPaneView === "catalog" && activeTab === "tools" && selectedTool ? (
            <div className="grid min-h-0 min-w-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_minmax(0,1fr)] gap-6 overflow-hidden xl:grid-cols-[minmax(0,1fr)_22rem] xl:grid-rows-1">
              <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
                <ToolDetailDocHeader tool={selectedTool} />
                <ScrollPane className="min-h-0 flex-1">
                  <ToolDetailDocBody tool={selectedTool} toolArgs={toolArgs} onToolArgChange={setToolArg} />
                </ScrollPane>
              </div>
              <ToolPlaygroundAside
                tool={selectedTool}
                instanceId={service.instance_id}
                toolArgs={toolArgs}
                toolArgsSchema={toolArgsSchema}
                onRun={() => props.onRunTool(selectedTool, serializeToolArgs(toolArgs, toolArgsSchema))}
              />
            </div>
          ) : (
            <ScrollPane className="flex-1">
              {rightPaneView === "service" ? (
                <ServiceOverviewPane
                  service={service}
                  description={description}
                  transport={transport}
                  launchLine={launchLine}
                  configArgs={configArgs}
                  endpoint={String(endpoint)}
                  availableToolCount={availableToolCount}
                  statusReport={statusReport}
                />
              ) : activeTab === "tools" ? (
                <PageEmpty title={t("noToolSelected")} description={t("serviceToolDetailsWillAppear")} onRefresh={loadDetail} />
              ) : activeTab === "resources" ? (
                selectedResource ? (
                  <ServiceResourceDetailPane resource={selectedResource} instanceId={service.instance_id} />
                ) : (
                  <PageEmpty title={t("noResourceSelected")} description={t("noResourceSelectedDescription")} onRefresh={loadDetail} />
                )
              ) : activeTab === "templates" ? (
                selectedTemplate ? (
                  <ServiceResourceTemplateDetailPane template={selectedTemplate} />
                ) : (
                  <PageEmpty title={t("noTemplateSelected")} description={t("noTemplateSelectedDescription")} onRefresh={loadDetail} />
                )
              ) : selectedPrompt ? (
                <ServicePromptDetailPane prompt={selectedPrompt} />
              ) : (
                <PageEmpty title={t("noPromptSelected")} description={t("noPromptSelectedDescription")} onRefresh={loadDetail} />
              )}
            </ScrollPane>
          )}
        </PanelCard>
      </TwoPanePage>

      <ServiceStatusActionsDialog
        busy={props.busy}
        open={statusDialogOpen}
        service={service}
        serviceStatus={service.status}
        onConnect={props.onConnect}
        onDelete={props.onDelete}
        onDisconnect={props.onDisconnect}
        onOpenChange={setStatusDialogOpen}
        onRestart={props.onRestart}
      />
    </>
  )
}

function ServicePreviewHeader({
  rightPaneView,
  activeTab,
  loading,
  service,
  selectedTool,
  selectedResource,
  selectedTemplate,
  selectedPrompt,
  toolCount,
  resourceCount,
  templateCount,
  promptCount,
  instanceId,
  onRefresh,
  onRun,
  onDetail,
  toolSearchQuery,
  onToolSearchQueryChange,
}: {
  rightPaneView: RightPaneView
  activeTab: CatalogTab
  loading: boolean
  service: ServiceInstance
  selectedTool: ToolInfo | null
  selectedResource: ResourceInfo | null
  selectedTemplate: ResourceTemplateInfo | null
  selectedPrompt: PromptInfo | null
  toolCount: number
  resourceCount: number
  templateCount: number
  promptCount: number
  instanceId: string
  onRefresh: () => void
  onRun?: () => void
  onDetail?: () => void
  toolSearchQuery: string
  onToolSearchQueryChange: (value: string) => void
}) {
  const { t } = useI18n()
  const title =
    rightPaneView === "service"
      ? service.service_name
      : activeTab === "tools"
        ? selectedTool?.name || t("toolsAvailable", { count: toolCount })
        : activeTab === "resources"
          ? selectedResource?.title || selectedResource?.name || selectedResource?.uri || t("resourcesAvailable", { count: resourceCount })
          : activeTab === "templates"
            ? selectedTemplate?.title || selectedTemplate?.name || (selectedTemplate ? resourceTemplateUri(selectedTemplate) : "") || t("templatesAvailable", { count: templateCount })
            : selectedPrompt?.title || selectedPrompt?.name || t("promptsAvailable", { count: promptCount })

  const copyPayload =
    rightPaneView === "service"
      ? service
      : activeTab === "tools"
        ? selectedTool
        : activeTab === "resources"
          ? selectedResource
          : activeTab === "templates"
            ? selectedTemplate
            : selectedPrompt

  async function onCopy() {
    if (!copyPayload) return
    await navigator.clipboard.writeText(JSON.stringify(copyPayload, null, 2))
    toast.success(t("copied"))
  }

  async function onReadResource() {
    if (!selectedResource) return
    try {
      const result = await readInstanceResource(instanceId, selectedResource.uri)
      await navigator.clipboard.writeText(JSON.stringify(result, null, 2))
      toast.success(t("resourceContentCopied"))
    } catch (err) {
      const message = err instanceof Error ? err.message : t("readResourceFailed")
      toast.error(message)
    }
  }

  const showToolHeader = rightPaneView === "catalog" && activeTab === "tools" && Boolean(selectedTool)

  return (
    <div className="flex flex-wrap items-center gap-3 border-b pb-2">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        {showToolHeader ? (
          <>
            <h2 className="shrink-0 truncate font-mono text-sm font-medium" title={selectedTool!.name}>
              {selectedTool!.name}
            </h2>
            <SearchBox placeholder={t("searchTools")} value={toolSearchQuery} onChange={onToolSearchQueryChange} />
          </>
        ) : (
          <div className="flex min-w-0 flex-col gap-1">
            <strong className="truncate font-mono text-sm font-medium" title={title}>
              {title}
            </strong>
          </div>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {rightPaneView === "catalog" && activeTab === "tools" && onRun ? (
          <Button size="sm" onClick={onRun}>
            <WrenchIcon data-icon="inline-start" />
            {t("run")}
          </Button>
        ) : null}
        {rightPaneView === "catalog" && activeTab === "tools" && onDetail ? (
          <Button size="sm" variant="outline" onClick={onDetail}>
            <EyeIcon data-icon="inline-start" />
            {t("details")}
          </Button>
        ) : null}
        {rightPaneView === "catalog" && activeTab === "resources" && selectedResource ? (
          <Button size="sm" variant="outline" onClick={onReadResource}>
            <FileIcon data-icon="inline-start" />
            {t("read")}
          </Button>
        ) : null}
        {copyPayload ? (
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

function ServiceOverviewPane({
  service,
  description,
  transport,
  launchLine,
  configArgs,
  endpoint,
  availableToolCount,
  statusReport,
}: {
  service: ServiceInstance
  description: string
  transport: string
  launchLine: string
  configArgs: string | null
  endpoint: string
  availableToolCount: number | undefined
  statusReport: InstanceStatus | null | undefined
}) {
  const { t } = useI18n()
  const capabilities = service.mcp?.capabilities
  const capabilityLabels = capabilities
    ? [
        capabilities.tools && t("tools"),
        capabilities.resources && t("resources"),
        capabilities.resourcesSubscribe && t("resourceSubscriptions"),
        capabilities.prompts && t("prompts"),
        capabilities.completions && t("completions"),
        capabilities.logging && t("logging"),
        capabilities.tasks && t("tasks"),
      ].filter((label): label is string => Boolean(label))
    : []

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <ServiceAuthPanel instanceId={service.instance_id} />
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
            <dt className="text-muted-foreground">{t("agentScope")}</dt>
            <dd className="font-mono">
              {service.scope.type === "store" ? t("store") : `${t("agent")} ${service.scope.agent_id}`}
            </dd>
          </div>
          {description ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("description")}</dt>
              <dd>{description}</dd>
            </div>
          ) : null}
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("launch")}</dt>
            <dd className="font-mono break-all">{launchLine}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("endpoint")}</dt>
            <dd className="font-mono break-all">{endpoint}</dd>
          </div>
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("transport")}</dt>
            <dd className="font-mono">{transport}{configArgs ? ` · ${configArgs}` : ""}</dd>
          </div>
          {availableToolCount !== undefined ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("availableTools")}</dt>
              <dd>{availableToolCount}</dd>
            </div>
          ) : null}
          {statusReport?.health_status ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("health")}</dt>
              <dd>{statusReport.health_status}</dd>
            </div>
          ) : null}
        </dl>
      </section>
      {service.mcp ? (
        <section className="border-b pb-4">
          <SectionHeading title={t("mcpServer")} titleAs="h2" className="border-b-0 pb-3" />
          <dl className="grid gap-3 text-sm">
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("serverImplementation")}</dt>
              <dd className="font-mono">
                {service.mcp.serverInfo.title || service.mcp.serverInfo.name} · {service.mcp.serverInfo.version}
              </dd>
            </div>
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("protocolVersion")}</dt>
              <dd className="font-mono">{service.mcp.protocolVersion}</dd>
            </div>
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("capabilities")}</dt>
              <dd className="flex flex-wrap gap-2">
                {capabilityLabels.map((label) => (
                  <Badge key={label} variant="outline">{label}</Badge>
                ))}
              </dd>
            </div>
            {service.mcp.instructions ? (
              <div className="grid gap-1">
                <dt className="text-muted-foreground">{t("serverInstructions")}</dt>
                <dd className="whitespace-pre-wrap">{service.mcp.instructions}</dd>
              </div>
            ) : null}
          </dl>
        </section>
      ) : null}
    </div>
  )
}

function ServiceResourceDetailPane({ resource, instanceId }: { resource: ResourceInfo; instanceId: string }) {
  const { t } = useI18n()
  const [content, setContent] = useState<unknown>(null)
  const [reading, setReading] = useState(false)
  const mimeType = resourceMimeType(resource)

  async function onRead() {
    setReading(true)
    try {
      const result = await readInstanceResource(instanceId, resource.uri)
      setContent(result)
    } catch (err) {
      const message = err instanceof Error ? err.message : t("readResourceFailed")
      toast.error(message)
    } finally {
      setReading(false)
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("resource")} titleAs="h2" className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("uri")}</dt>
            <dd className="font-mono break-all">{resource.uri}</dd>
          </div>
          {resource.name ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("name")}</dt>
              <dd>{resource.name}</dd>
            </div>
          ) : null}
          {resource.description ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("description")}</dt>
              <dd>{resource.description}</dd>
            </div>
          ) : null}
          {mimeType ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("mimeType")}</dt>
              <dd className="font-mono">{mimeType}</dd>
            </div>
          ) : null}
        </dl>
        <div className="mt-4">
          <Button size="sm" variant="outline" onClick={onRead} disabled={reading}>
            <FileIcon data-icon="inline-start" />
            {reading ? t("reading") : t("readResource")}
          </Button>
        </div>
      </section>
      {content ? (
        <section className="pb-2">
          <SectionHeading title={t("content")} titleAs="h2" className="border-b-0 pb-3" />
          <JsonBlock value={content} />
        </section>
      ) : null}
    </div>
  )
}

function ServiceResourceTemplateDetailPane({ template }: { template: ResourceTemplateInfo }) {
  const { t } = useI18n()
  const uriTemplate = resourceTemplateUri(template)
  const mimeType = resourceTemplateMimeType(template)
  const annotations = getResourceTemplateAnnotations(template)
  const meta = getResourceTemplateMeta(template)

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("resourceTemplate")} titleAs="h2" className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("uriTemplate")}</dt>
            <dd className="font-mono break-all">{uriTemplate || "-"}</dd>
          </div>
          {template.name ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("name")}</dt>
              <dd>{template.name}</dd>
            </div>
          ) : null}
          {template.title ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("titleLabel")}</dt>
              <dd>{template.title}</dd>
            </div>
          ) : null}
          {template.description ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("description")}</dt>
              <dd>{template.description}</dd>
            </div>
          ) : null}
          {mimeType ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("mimeType")}</dt>
              <dd className="font-mono">{mimeType}</dd>
            </div>
          ) : null}
        </dl>
      </section>
      {hasResourceTemplateAnnotations(template) ? (
        <section className="border-b pb-4">
          <SectionHeading title={t("annotations")} titleAs="h2" className="border-b-0 pb-3" />
          <JsonBlock value={annotations} />
        </section>
      ) : null}
      {hasResourceTemplateMeta(template) ? (
        <section className="pb-2">
          <SectionHeading title={t("meta")} titleAs="h2" className="border-b-0 pb-3" />
          <JsonBlock value={meta} />
        </section>
      ) : null}
    </div>
  )
}

function ServicePromptDetailPane({ prompt }: { prompt: PromptInfo }) {
  const { t } = useI18n()
  const argCount = promptArgCount(prompt)

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading title={t("promptLabel")} titleAs="h2" className="border-b-0 pb-3" />
        <dl className="grid gap-3 text-sm">
          <div className="grid gap-1">
            <dt className="text-muted-foreground">{t("name")}</dt>
            <dd className="font-mono">{prompt.name}</dd>
          </div>
          {prompt.title ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("titleLabel")}</dt>
              <dd>{prompt.title}</dd>
            </div>
          ) : null}
          {prompt.description ? (
            <div className="grid gap-1">
              <dt className="text-muted-foreground">{t("description")}</dt>
              <dd>{prompt.description}</dd>
            </div>
          ) : null}
        </dl>
      </section>
      <section className="pb-2">
        <SectionHeading title={t("arguments")} titleAs="h2" badge={argCount || undefined} className="border-b-0 pb-3" />
        {prompt.arguments ? (
          <JsonBlock value={prompt.arguments} />
        ) : (
          <p className="rounded-lg border border-dashed bg-muted/10 px-3 py-4 text-sm text-muted-foreground">
            {t("noPromptArguments")}
          </p>
        )}
      </section>
    </div>
  )
}
