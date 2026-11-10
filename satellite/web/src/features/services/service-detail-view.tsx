import { useEffect, useMemo, useState, type ReactNode } from "react"
import { FileStackIcon, ScrollTextIcon } from "lucide-react"
import { useDefaultLayout } from "react-resizable-panels"
import { toast } from "sonner"

import { DefaultToolIcon } from "@/components/shared/default-tool-icon"
import { McpIcon } from "@/components/shared/mcp-icon"
import { PageEmpty, PageError } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import {
  breadcrumbUppercase,
  WorkspaceBreadcrumb,
} from "@/components/shared/workspace-page-header"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import {
  ToolDetailDocBody,
  ToolDetailDocHeader,
  ToolPlaygroundAside,
} from "@/components/shared/tool-detail-playground"
import { toolDetailPlaygroundGrid } from "@/components/shared/tool-detail-section-layout"
import { Badge } from "@/components/ui/badge"
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import {
  useServiceAuthQuery,
  useServiceDetailQuery,
  useServicePromptsQuery,
  useServiceProjectedToolsQuery,
  useServiceResourceTemplatesQuery,
  useServiceResourcesQuery,
  useServiceStatusQuery,
} from "@/features/services/queries"
import { useToolArgsForm } from "@/features/tools/use-tool-args-form"
import { serializeToolArgs } from "@/lib/tool-args"
import { getOriginalToolName, getToolSchema, toolKey } from "@/lib/tool-info"
import {
  resourceTemplateKey,
  resourceTemplateMimeType,
  resourceTemplateUri,
} from "@/lib/resource-template-info"
import type { PromptInfo, ResourceInfo, ServiceInstance, ServiceState, ToolInfo } from "@/lib/api"
import { formatServiceLaunchLine } from "@/lib/service-info"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

import {
  ServiceOverviewPane,
  promptArgCount,
  ServicePreviewActions,
  ServicePromptDetailPane,
  ServiceResourceDetailPane,
  ServiceResourceTemplateDetailPane,
  type ResourceSubTab,
  type ServiceDetailTab,
} from "@/features/services/service-panels"

function resourceKey(resource: ResourceInfo) {
  return resource.uri
}

function promptKey(prompt: PromptInfo) {
  return prompt.name
}

function resourceMimeType(resource: ResourceInfo) {
  return String(resource.mimeType || "").trim()
}

const CATALOG_SPLIT_LAYOUT_ID = "service-detail-catalog"
const CATALOG_LIST_PANEL_ID = "list"
const CATALOG_DETAIL_PANEL_ID = "detail"

function CatalogRowIcon({ icons, fallback }: { icons: unknown; fallback: ReactNode }) {
  return (
    <span
      aria-hidden
      className="flex size-8 shrink-0 items-center justify-center rounded-md border border-border/50 bg-muted/60 text-muted-foreground"
    >
      <McpIcon icons={icons} fallback={fallback} className="size-4" />
    </span>
  )
}

function CatalogSplitLayout(props: {
  list: ReactNode
  detail: ReactNode
  defaultLayout: ReturnType<typeof useDefaultLayout>["defaultLayout"]
  onLayoutChanged: ReturnType<typeof useDefaultLayout>["onLayoutChanged"]
}) {
  return (
    <ResizablePanelGroup
      id={CATALOG_SPLIT_LAYOUT_ID}
      orientation="horizontal"
      className="min-h-0 flex-1"
      defaultLayout={props.defaultLayout}
      onLayoutChanged={props.onLayoutChanged}
    >
      <ResizablePanel
        id={CATALOG_LIST_PANEL_ID}
        defaultSize="16rem"
        minSize="11rem"
        maxSize="40%"
        groupResizeBehavior="preserve-pixel-size"
        className="min-h-0"
      >
        <div className="flex h-full min-h-0 flex-col gap-3 overflow-hidden pr-3">
          {props.list}
        </div>
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel
        id={CATALOG_DETAIL_PANEL_ID}
        minSize="40%"
        className="min-h-0"
      >
        <div className="flex h-full min-h-0 flex-col overflow-hidden pl-3">{props.detail}</div>
      </ResizablePanel>
    </ResizablePanelGroup>
  )
}

export function ServiceDetailView(props: {
  service: ServiceInstance
  busy: string | null
  refreshToken?: number
  onBack: () => void
  onRunTool: (tool: ToolInfo, args: Record<string, unknown>) => void
  isToolRunning?: (tool: ToolInfo) => boolean
  onToolDetail: (
    tool: ToolInfo,
    service: ServiceInstance,
    statusReport?: ServiceState | null,
  ) => void
  onConnect: () => void
  onDisconnect: () => void
  onRestart: () => void
  onDelete: () => void
}) {
  const { t } = useI18n()
  const [detailError, setDetailError] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState<ServiceDetailTab>("service")
  const [resourceSubTab, setResourceSubTab] = useState<ResourceSubTab>("items")
  const [selectedToolKey, setSelectedToolKey] = useState<string | null>(null)
  const [selectedResourceKey, setSelectedResourceKey] = useState<string | null>(null)
  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string | null>(null)
  const [selectedPromptKey, setSelectedPromptKey] = useState<string | null>(null)
  const [playgroundOpen, setPlaygroundOpen] = useState(true)
  const detailQuery = useServiceDetailQuery(props.service)
  const authQuery = useServiceAuthQuery(props.service)
  const statusQuery = useServiceStatusQuery(props.service)
  const resourcesQuery = useServiceResourcesQuery(props.service)
  const resourceTemplatesQuery = useServiceResourceTemplatesQuery(props.service)
  const promptsQuery = useServicePromptsQuery(props.service)
  const detail = detailQuery.data
  const statusReport = statusQuery.data
  const error = detailQuery.error || detailError
  const errorMessage =
    error instanceof Error ? error.message : error ? String(error) : t("serviceDetailLoadFailed")
  const loading =
    detailQuery.isFetching ||
    statusQuery.isFetching ||
    resourcesQuery.isFetching ||
    resourceTemplatesQuery.isFetching ||
    promptsQuery.isFetching
  const service = detail || props.service
  const endpoint = service.url || service.command || "-"
  // Prefer the override-projected list (same surface the global tools page
  // uses); fall back to the raw detail payload until it arrives.
  const projectedToolsQuery = useServiceProjectedToolsQuery(props.service)
  const tools = projectedToolsQuery.data?.length
    ? projectedToolsQuery.data
    : service.tools || []
  const resources = resourcesQuery.data || []
  const resourceTemplates = resourceTemplatesQuery.data || []
  const prompts = promptsQuery.data || []
  const serviceConfig = service.effective_config
  const transport = service.transport
  const launchLine = formatServiceLaunchLine(service)
  const description = String(serviceConfig?.description || "").trim()
  const configArgs = Array.isArray(serviceConfig?.args)
    ? serviceConfig.args.map(String).join(" ")
    : null
  const selectedTool = useMemo(() => {
    if (!tools.length) return null
    return tools.find((tool) => toolKey(service.instance_id, tool) === selectedToolKey) || tools[0]
  }, [selectedToolKey, service.instance_id, tools])
  const selectedResource = useMemo(() => {
    if (!resources.length) return null
    return (
      resources.find((resource) => resourceKey(resource) === selectedResourceKey) || resources[0]
    )
  }, [resources, selectedResourceKey])
  const selectedTemplate = useMemo(() => {
    if (!resourceTemplates.length) return null
    return (
      resourceTemplates.find((template) => resourceTemplateKey(template) === selectedTemplateKey) ||
      resourceTemplates[0]
    )
  }, [resourceTemplates, selectedTemplateKey])
  const selectedPrompt = useMemo(() => {
    if (!prompts.length) return null
    return prompts.find((prompt) => promptKey(prompt) === selectedPromptKey) || prompts[0]
  }, [prompts, selectedPromptKey])
  const {
    values: toolArgs,
    setField: setToolArg,
    schema: toolArgsSchema,
  } = useToolArgsForm(selectedTool)
  const runningSelectedTool = Boolean(selectedTool && props.isToolRunning?.(selectedTool))

  useEffect(() => {
    if (!tools.length) {
      setSelectedToolKey(null)
      return
    }
    if (
      !selectedToolKey ||
      !tools.some((tool) => toolKey(service.instance_id, tool) === selectedToolKey)
    ) {
      setSelectedToolKey(toolKey(service.instance_id, tools[0]))
    }
  }, [selectedToolKey, service.instance_id, tools])

  useEffect(() => {
    if (!resources.length) {
      setSelectedResourceKey(null)
      return
    }
    if (
      !selectedResourceKey ||
      !resources.some((resource) => resourceKey(resource) === selectedResourceKey)
    ) {
      setSelectedResourceKey(resourceKey(resources[0]))
    }
  }, [resources, selectedResourceKey])

  useEffect(() => {
    if (!resourceTemplates.length) {
      setSelectedTemplateKey(null)
      return
    }
    if (
      !selectedTemplateKey ||
      !resourceTemplates.some((template) => resourceTemplateKey(template) === selectedTemplateKey)
    ) {
      setSelectedTemplateKey(resourceTemplateKey(resourceTemplates[0]))
    }
  }, [resourceTemplates, selectedTemplateKey])

  useEffect(() => {
    if (resourceSubTab === "items" && !resources.length && resourceTemplates.length) {
      setResourceSubTab("templates")
    } else if (resourceSubTab === "templates" && !resourceTemplates.length && resources.length) {
      setResourceSubTab("items")
    }
  }, [resourceSubTab, resourceTemplates.length, resources.length])

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
      const [nextDetail] = await Promise.all([detailQuery.refetch(), statusQuery.refetch()])
      if (nextDetail.error) throw nextDetail.error
    } catch (err) {
      const message = err instanceof Error ? err.message : t("serviceDetailLoadFailed")
      if (!detailQuery.error) setDetailError(message)
      toast.error(message)
    }
  }

  async function loadCatalog() {
    if (activeTab === "resources") {
      await Promise.all([resourcesQuery.refetch(), resourceTemplatesQuery.refetch()])
      return
    }
    if (activeTab === "prompts") await promptsQuery.refetch()
  }

  function refreshCurrentView() {
    if (activeTab === "service") {
      return Promise.all([loadDetail(), authQuery.refetch()])
    }
    return activeTab === "tools" ? loadDetail() : loadCatalog()
  }

  useEffect(() => {
    setActiveTab("service")
    setResourceSubTab("items")
  }, [props.service.instance_id])

  useEffect(() => {
    void loadDetail()
  }, [props.refreshToken, props.service.instance_id])

  useEffect(() => {
    if (activeTab === "prompts" || activeTab === "resources") void loadCatalog()
  }, [activeTab, props.service.instance_id])

  const catalogSplitClassName =
    "mt-0 flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
  const { defaultLayout: catalogDefaultLayout, onLayoutChanged: onCatalogLayoutChanged } =
    useDefaultLayout({
      id: CATALOG_SPLIT_LAYOUT_ID,
      panelIds: [CATALOG_LIST_PANEL_ID, CATALOG_DETAIL_PANEL_ID],
      storage: localStorage,
    })

  const previewActionsProps = {
    activeTab,
    resourceSubTab,
    loading,
    service,
    selectedTool,
    selectedResource,
    selectedTemplate,
    selectedPrompt,
    onRefresh: refreshCurrentView,
    onConnect: props.onConnect,
    onDisconnect: props.onDisconnect,
    busy: props.busy,
    onRun:
      activeTab === "tools" && selectedTool
        ? () => props.onRunTool(selectedTool, serializeToolArgs(toolArgs, toolArgsSchema))
        : undefined,
    runningTool: runningSelectedTool,
    onDetail:
      activeTab === "tools" && selectedTool
        ? () => props.onToolDetail(selectedTool, service, statusReport)
        : undefined,
    playgroundOpen,
    onTogglePlayground:
      activeTab === "tools" && selectedTool
        ? () => setPlaygroundOpen((open) => !open)
        : undefined,
  }

  function renderServiceDetail() {
    return (
      <div className="flex min-h-0 flex-col overflow-hidden">
        {error ? (
          <ScrollPane className="flex-1">
            <PageError
              title={t("serviceDetailLoadFailed")}
              message={errorMessage}
              onRefresh={refreshCurrentView}
            />
          </ScrollPane>
        ) : (
          <ScrollPane className="flex-1">
            <div className="rounded-lg bg-muted/15 p-4">
              <ServiceOverviewPane
                service={service}
                description={description}
                transport={transport}
                launchLine={launchLine}
                configArgs={configArgs}
                endpoint={String(endpoint)}
              />
            </div>
          </ScrollPane>
        )}
      </div>
    )
  }

  function renderCatalogDetail() {
    return (
      <div className="flex min-h-0 flex-col overflow-hidden">
        {error ? (
          <ScrollPane className="flex-1">
            <PageError
              title={t("serviceDetailLoadFailed")}
              message={errorMessage}
              onRefresh={refreshCurrentView}
            />
          </ScrollPane>
        ) : activeTab === "tools" && selectedTool ? (
          <div
            className={cn(
              toolDetailPlaygroundGrid,
              !playgroundOpen && "@min-[42rem]:grid-cols-1",
            )}
          >
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ScrollPane className="min-h-0 flex-1">
                <ToolDetailDocHeader tool={selectedTool} />
                <ToolDetailDocBody
                  tool={selectedTool}
                  toolArgs={toolArgs}
                  onToolArgChange={setToolArg}
                  serviceName={service.service_name}
                  scope={service.scope}
                  originalToolName={getOriginalToolName(selectedTool)}
                />
              </ScrollPane>
            </div>
            {playgroundOpen ? (
              <ToolPlaygroundAside
                tool={selectedTool}
                instanceId={service.instance_id}
                toolArgs={toolArgs}
                toolArgsSchema={toolArgsSchema}
                running={runningSelectedTool}
                onRun={() =>
                  props.onRunTool(selectedTool, serializeToolArgs(toolArgs, toolArgsSchema))
                }
              />
            ) : null}
          </div>
        ) : (
          <ScrollPane className="flex-1">
            {activeTab === "tools" ? (
              <PageEmpty
                title={t("noToolSelected")}
                description={t("serviceToolDetailsWillAppear")}
                onRefresh={refreshCurrentView}
              />
            ) : activeTab === "resources" ? (
              resourceSubTab === "templates" ? (
                selectedTemplate ? (
                  <ServiceResourceTemplateDetailPane
                    template={selectedTemplate}
                    service={service}
                  />
                ) : (
                  <PageEmpty
                    title={t("noTemplateSelected")}
                    description={t("noTemplateSelectedDescription")}
                    onRefresh={refreshCurrentView}
                  />
                )
              ) : selectedResource ? (
                <ServiceResourceDetailPane resource={selectedResource} service={service} />
              ) : (
                <PageEmpty
                  title={t("noResourceSelected")}
                  description={t("noResourceSelectedDescription")}
                  onRefresh={refreshCurrentView}
                />
              )
            ) : selectedPrompt ? (
              <ServicePromptDetailPane prompt={selectedPrompt} service={service} />
            ) : (
              <PageEmpty title={t("noPromptSelected")} onRefresh={refreshCurrentView} />
            )}
          </ScrollPane>
        )}
      </div>
    )
  }

  const headerSubtitle = [transport, description].filter(Boolean).join(" · ")
  const scopeLabel = breadcrumbUppercase(
    service.scope.type === "store"
      ? t("store")
      : service.scope.type === "agent"
        ? service.scope.agent_id
        : t("filterAll"),
  )

  return (
    <>
      <WorkspaceBreadcrumb
        ariaLabel={t("serviceDetail")}
        crumbs={[
          { label: t("servicesWorkspaceTitle"), onClick: props.onBack },
          { label: scopeLabel, onClick: props.onBack },
          { label: service.service_name },
        ]}
        subtitle={headerSubtitle || undefined}
      />

      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col gap-0 overflow-hidden p-0">
        <Tabs
          value={activeTab}
          onValueChange={(value) => setActiveTab(value as ServiceDetailTab)}
          className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden px-4 pt-2 pb-2"
        >
          <div className="flex min-w-0 shrink-0 items-center gap-3">
            <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
              <TabsTrigger value="service">{t("service")}</TabsTrigger>
              <TabsTrigger value="prompts">{t("prompts")}</TabsTrigger>
              <TabsTrigger value="tools">{t("tools")}</TabsTrigger>
              <TabsTrigger value="resources">{t("resources")}</TabsTrigger>
            </TabsList>
            <ServicePreviewActions {...previewActionsProps} className="ml-auto" />
          </div>

          <TabsContent
            value="service"
            className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
          >
            {renderServiceDetail()}
          </TabsContent>

          <TabsContent value="tools" className={catalogSplitClassName}>
            <CatalogSplitLayout
              defaultLayout={catalogDefaultLayout}
              onLayoutChanged={onCatalogLayoutChanged}
              list={
                tools.length ? (
                  <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                    {tools.map((tool) => {
                      const key = toolKey(service.instance_id, tool)
                      const schema = getToolSchema(tool) as {
                        properties?: Record<string, unknown>
                      }
                      const paramCount = Object.keys(schema.properties || {}).length
                      return (
                        <SelectableRowButton
                          key={key}
                          leading={
                            <CatalogRowIcon
                              icons={tool.icons}
                              fallback={<DefaultToolIcon className="size-4" />}
                            />
                          }
                          meta={t("paramCount", { count: paramCount })}
                          onClick={() => setSelectedToolKey(key)}
                          selected={key === toolKey(service.instance_id, selectedTool || tool)}
                          title={tool.name}
                        />
                      )
                    })}
                  </ScrollPane>
                ) : (
                  <PageEmpty
                    title={t("noToolsFound")}
                    description={t("noToolsFoundDescription")}
                    onRefresh={refreshCurrentView}
                  />
                )
              }
              detail={renderCatalogDetail()}
            />
          </TabsContent>

          <TabsContent value="prompts" className={catalogSplitClassName}>
            <CatalogSplitLayout
              defaultLayout={catalogDefaultLayout}
              onLayoutChanged={onCatalogLayoutChanged}
              list={
                prompts.length ? (
                  <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                    {prompts.map((prompt) => {
                      const key = promptKey(prompt)
                      const argCount = promptArgCount(prompt)
                      return (
                        <SelectableRowButton
                          key={key}
                          leading={
                            <CatalogRowIcon
                              icons={prompt.icons}
                              fallback={<ScrollTextIcon className="size-4" />}
                            />
                          }
                          meta={
                            prompt.description ||
                            (argCount ? t("paramCount", { count: argCount }) : t("noArgs"))
                          }
                          onClick={() => setSelectedPromptKey(key)}
                          selected={key === promptKey(selectedPrompt || prompt)}
                          title={prompt.title || prompt.name}
                        />
                      )
                    })}
                  </ScrollPane>
                ) : (
                  <PageEmpty title={t("noPromptsFound")} onRefresh={refreshCurrentView} />
                )
              }
              detail={renderCatalogDetail()}
            />
          </TabsContent>

          <TabsContent value="resources" className={catalogSplitClassName}>
            <CatalogSplitLayout
              defaultLayout={catalogDefaultLayout}
              onLayoutChanged={onCatalogLayoutChanged}
              list={
                resources.length || resourceTemplates.length ? (
                  <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2">
                    {resources.map((resource) => {
                      const key = resourceKey(resource)
                      const mimeType = resourceMimeType(resource)
                      return (
                        <SelectableRowButton
                          key={`resource:${key}`}
                          leading={
                            <CatalogRowIcon
                              icons={resource.icons}
                              fallback={<FileStackIcon className="size-4" />}
                            />
                          }
                          meta={mimeType || resource.uri}
                          onClick={() => {
                            setResourceSubTab("items")
                            setSelectedResourceKey(key)
                          }}
                          selected={
                            resourceSubTab === "items" &&
                            key === resourceKey(selectedResource || resource)
                          }
                          title={resource.title || resource.name || resource.uri}
                        />
                      )
                    })}
                    {resourceTemplates.map((template) => {
                      const key = resourceTemplateKey(template)
                      const mimeType = resourceTemplateMimeType(template)
                      const uriTemplate = resourceTemplateUri(template)
                      return (
                        <SelectableRowButton
                          key={`template:${key}`}
                          leading={
                            <CatalogRowIcon
                              icons={template.icons}
                              fallback={<FileStackIcon className="size-4" />}
                            />
                          }
                          meta={mimeType || uriTemplate}
                          onClick={() => {
                            setResourceSubTab("templates")
                            setSelectedTemplateKey(key)
                          }}
                          selected={
                            resourceSubTab === "templates" &&
                            key === resourceTemplateKey(selectedTemplate || template)
                          }
                          title={template.title || template.name || uriTemplate}
                          trailing={<Badge variant="outline">{t("templates")}</Badge>}
                        />
                      )
                    })}
                  </ScrollPane>
                ) : (
                  <PageEmpty
                    title={t("noResourcesFound")}
                    description={t("noResourcesFoundDescription")}
                    onRefresh={refreshCurrentView}
                  />
                )
              }
              detail={renderCatalogDetail()}
            />
          </TabsContent>
        </Tabs>
      </PanelCard>
    </>
  )
}
