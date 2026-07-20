import { useEffect, useMemo, useState } from "react";
import {
  ClipboardIcon,
  EyeIcon,
  FileIcon,
  LayoutTemplateIcon,
  RefreshCwIcon,
  WrenchIcon,
} from "lucide-react";
import { toast } from "sonner";

import { JsonBlock } from "@/components/shared/json-block";
import { MetricGrid, MetricTile } from "@/components/shared/metric-grid";
import { PageEmpty, PageError } from "@/components/shared/page-states";
import { PanelCard } from "@/components/shared/panel-card";
import { ScrollPane } from "@/components/shared/scroll-pane";
import { SearchBox } from "@/components/shared/search-box";
import { SectionHeading } from "@/components/shared/section-heading";
import { SelectableRowButton } from "@/components/shared/selectable-row-button";
import {
  CatalogTabTrigger,
  CatalogTabsList,
} from "@/components/shared/catalog-tabs-list";
import {
  ToolDetailDocBody,
  ToolDetailDocHeader,
  ToolPlaygroundAside,
} from "@/components/shared/tool-detail-playground";
import { TwoPanePage } from "@/components/shared/two-pane-page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useServiceDetailQuery,
  useServicePromptsQuery,
  useServiceResourceTemplatesQuery,
  useServiceResourcesQuery,
  useServiceStatusQuery,
} from "@/features/services/queries";
import { ServiceAuthPanel } from "@/features/services/service-auth-panel";
import { ServiceStatusActionsDialog } from "@/features/services/service-status-actions-dialog";
import { useToolArgsForm } from "@/features/tools/use-tool-args-form";
import { serializeToolArgs, type ToolSchema } from "@/lib/tool-args";
import { getToolSchema, toolKey } from "@/lib/tool-info";
import {
  getResourceTemplateAnnotations,
  getResourceTemplateMeta,
  hasResourceTemplateAnnotations,
  hasResourceTemplateMeta,
  resourceTemplateKey,
  resourceTemplateMimeType,
  resourceTemplateUri,
} from "@/lib/resource-template-info";
import {
  readInstanceResource,
  type PromptInfo,
  type ResourceInfo,
  type ResourceTemplateInfo,
  type ServiceInstance,
  type ServiceState,
  type ToolInfo,
} from "@/lib/api";
import { formatServiceLaunchLine } from "@/lib/service-info";
import { useI18n } from "@/lib/i18n-context";
import { cn } from "@/lib/utils";

import {
  ServiceOverviewPane,
  promptArgCount,
  ServicePreviewHeader,
  ServicePromptDetailPane,
  ServiceResourceDetailPane,
  ServiceResourceTemplateDetailPane,
  type CatalogTab,
  type ResourceSubTab,
  type RightPaneView,
} from "@/features/services/service-panels";

function resourceKey(resource: ResourceInfo) {
  return resource.uri;
}

function promptKey(prompt: PromptInfo) {
  return prompt.name;
}

function resourceMimeType(resource: ResourceInfo) {
  return String(resource.mimeType || "").trim();
}

export function ServiceDetailView(props: {
  service: ServiceInstance;
  busy: string | null;
  refreshToken?: number;
  onBack: () => void;
  onRunTool: (tool: ToolInfo, args: Record<string, unknown>) => void;
  isToolRunning?: (tool: ToolInfo) => boolean;
  onToolDetail: (
    tool: ToolInfo,
    service: ServiceInstance,
    statusReport?: ServiceState | null,
  ) => void;
  onConnect: () => void;
  onDisconnect: () => void;
  onRestart: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  const [detailError, setDetailError] = useState<string | null>(null);
  const [rightPaneView, setRightPaneView] = useState<RightPaneView>("service");
  const [activeTab, setActiveTab] = useState<CatalogTab>("tools");
  const [resourceSubTab, setResourceSubTab] = useState<ResourceSubTab>("items");
  const [selectedToolKey, setSelectedToolKey] = useState<string | null>(null);
  const [selectedResourceKey, setSelectedResourceKey] = useState<string | null>(
    null,
  );
  const [selectedTemplateKey, setSelectedTemplateKey] = useState<string | null>(
    null,
  );
  const [selectedPromptKey, setSelectedPromptKey] = useState<string | null>(
    null,
  );
  const [statusDialogOpen, setStatusDialogOpen] = useState(false);
  const [toolSearchQuery, setToolSearchQuery] = useState("");
  const detailQuery = useServiceDetailQuery(props.service.instance_id);
  const statusQuery = useServiceStatusQuery(props.service.instance_id);
  const resourcesQuery = useServiceResourcesQuery(props.service.instance_id);
  const resourceTemplatesQuery = useServiceResourceTemplatesQuery(
    props.service.instance_id,
  );
  const promptsQuery = useServicePromptsQuery(props.service.instance_id);
  const detail = detailQuery.data;
  const statusReport = statusQuery.data;
  const error = detailQuery.error || detailError;
  const errorMessage =
    error instanceof Error
      ? error.message
      : error
        ? String(error)
        : t("serviceDetailLoadFailed");
  const loading =
    detailQuery.isFetching ||
    statusQuery.isFetching ||
    resourcesQuery.isFetching ||
    resourceTemplatesQuery.isFetching ||
    promptsQuery.isFetching;
  const service = detail || props.service;
  const endpoint = service.url || service.command || "-";
  const tools = service.tools || [];
  const filteredTools = useMemo(() => {
    const query = toolSearchQuery.trim().toLowerCase();
    if (!query) return tools;
    return tools.filter((tool) =>
      `${tool.name} ${tool.description || ""}`.toLowerCase().includes(query),
    );
  }, [toolSearchQuery, tools]);
  const resources = resourcesQuery.data || [];
  const resourceTemplates = resourceTemplatesQuery.data || [];
  const prompts = promptsQuery.data || [];
  const serviceConfig = service.effective_config;
  const transport = service.transport;
  const launchLine = formatServiceLaunchLine(service);
  const description = String(serviceConfig?.description || "").trim();
  const configArgs = Array.isArray(serviceConfig?.args)
    ? serviceConfig.args.map(String).join(" ")
    : null;
  const availableToolCount = statusReport?.tools.items.filter(
    (item) => item.availability === "available",
  ).length;
  const selectedTool = useMemo(() => {
    if (!tools.length) return null;
    return (
      tools.find(
        (tool) => toolKey(service.instance_id, tool) === selectedToolKey,
      ) || tools[0]
    );
  }, [selectedToolKey, service.instance_id, tools]);
  const selectedResource = useMemo(() => {
    if (!resources.length) return null;
    return (
      resources.find(
        (resource) => resourceKey(resource) === selectedResourceKey,
      ) || resources[0]
    );
  }, [resources, selectedResourceKey]);
  const selectedTemplate = useMemo(() => {
    if (!resourceTemplates.length) return null;
    return (
      resourceTemplates.find(
        (template) => resourceTemplateKey(template) === selectedTemplateKey,
      ) || resourceTemplates[0]
    );
  }, [resourceTemplates, selectedTemplateKey]);
  const selectedPrompt = useMemo(() => {
    if (!prompts.length) return null;
    return (
      prompts.find((prompt) => promptKey(prompt) === selectedPromptKey) ||
      prompts[0]
    );
  }, [prompts, selectedPromptKey]);
  const {
    values: toolArgs,
    setField: setToolArg,
    schema: toolArgsSchema,
  } = useToolArgsForm(selectedTool);
  const runningSelectedTool = Boolean(
    selectedTool && props.isToolRunning?.(selectedTool),
  );

  useEffect(() => {
    if (!tools.length) {
      setSelectedToolKey(null);
      return;
    }
    if (
      !selectedToolKey ||
      !tools.some(
        (tool) => toolKey(service.instance_id, tool) === selectedToolKey,
      )
    ) {
      setSelectedToolKey(toolKey(service.instance_id, tools[0]));
    }
  }, [selectedToolKey, service.instance_id, tools]);

  useEffect(() => {
    if (!resources.length) {
      setSelectedResourceKey(null);
      return;
    }
    if (
      !selectedResourceKey ||
      !resources.some(
        (resource) => resourceKey(resource) === selectedResourceKey,
      )
    ) {
      setSelectedResourceKey(resourceKey(resources[0]));
    }
  }, [resources, selectedResourceKey]);

  useEffect(() => {
    if (!resourceTemplates.length) {
      setSelectedTemplateKey(null);
      return;
    }
    if (
      !selectedTemplateKey ||
      !resourceTemplates.some(
        (template) => resourceTemplateKey(template) === selectedTemplateKey,
      )
    ) {
      setSelectedTemplateKey(resourceTemplateKey(resourceTemplates[0]));
    }
  }, [resourceTemplates, selectedTemplateKey]);

  useEffect(() => {
    if (!prompts.length) {
      setSelectedPromptKey(null);
      return;
    }
    if (
      !selectedPromptKey ||
      !prompts.some((prompt) => promptKey(prompt) === selectedPromptKey)
    ) {
      setSelectedPromptKey(promptKey(prompts[0]));
    }
  }, [prompts, selectedPromptKey]);

  async function loadDetail() {
    try {
      setDetailError(null);
      const [nextDetail] = await Promise.all([
        detailQuery.refetch(),
        statusQuery.refetch(),
        resourcesQuery.refetch(),
        resourceTemplatesQuery.refetch(),
        promptsQuery.refetch(),
      ]);
      if (nextDetail.error) throw nextDetail.error;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : t("serviceDetailLoadFailed");
      if (!detailQuery.error) setDetailError(message);
      toast.error(message);
    }
  }

  useEffect(() => {
    setRightPaneView("service");
    setToolSearchQuery("");
    setResourceSubTab("items");
  }, [props.service.instance_id]);

  useEffect(() => {
    void loadDetail();
  }, [props.refreshToken, props.service.instance_id]);

  return (
    <>
      <TwoPanePage variant="full" className="h-full min-h-0 flex-1 gap-4">
        <PanelCard className="@container h-full min-h-0">
          <section className="flex flex-col gap-3 border-b pb-4">
            <div className="min-w-0">
              <p className="font-mono text-xs uppercase text-muted-foreground">
                {t("service")}
              </p>
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
              <p
                className="mt-1 truncate font-mono text-xs text-muted-foreground"
                title={launchLine}
              >
                {launchLine}
              </p>
              {description ? (
                <p
                  className="mt-2 line-clamp-2 text-sm text-muted-foreground"
                  title={description}
                >
                  {description}
                </p>
              ) : null}
            </div>
          </section>

          <Tabs
            value={activeTab}
            onValueChange={(value) => {
              setActiveTab(value as CatalogTab);
              setRightPaneView("catalog");
            }}
            className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
          >
            <CatalogTabsList>
              <CatalogTabTrigger
                value="tools"
                label={t("tools")}
                variant="text"
              />
              <CatalogTabTrigger
                value="resources"
                label={t("resources")}
                variant="text"
              />
              <CatalogTabTrigger
                value="prompts"
                label={t("prompts")}
                variant="text"
              />
            </CatalogTabsList>

            <TabsContent
              value="tools"
              className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
            >
              <SectionHeading
                title={t("toolList")}
                titleAs="h2"
                description={t("itemsCount", { count: filteredTools.length })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {tools.length ? (
                filteredTools.length ? (
                  <ScrollPane
                    className="flex-1"
                    innerClassName="flex flex-col gap-2"
                  >
                    {filteredTools.map((tool) => {
                      const key = toolKey(service.instance_id, tool);
                      const schema = getToolSchema(tool) as {
                        properties?: Record<string, unknown>;
                        required?: string[];
                      };
                      const paramCount = Object.keys(
                        schema.properties || {},
                      ).length;
                      return (
                        <SelectableRowButton
                          key={key}
                          meta={t("paramCount", { count: paramCount })}
                          onClick={() => {
                            setSelectedToolKey(key);
                            setRightPaneView("catalog");
                          }}
                          selected={
                            rightPaneView === "catalog" &&
                            key ===
                              toolKey(service.instance_id, selectedTool || tool)
                          }
                          title={tool.name}
                          trailing={
                            schema.required?.length ? (
                              <Badge variant="outline">
                                {schema.required.length}
                              </Badge>
                            ) : null
                          }
                        />
                      );
                    })}
                  </ScrollPane>
                ) : (
                  <PageEmpty
                    title={t("noMatchingTools")}
                    description={t("noMatchingToolsDescription")}
                    onRefresh={loadDetail}
                  />
                )
              ) : (
                <PageEmpty
                  title={t("noToolsFound")}
                  description={t("noToolsFoundDescription")}
                  onRefresh={loadDetail}
                />
              )}
            </TabsContent>

            <TabsContent
              value="resources"
              className="mt-0 flex min-h-0 flex-1 flex-col overflow-hidden"
            >
              <Tabs
                value={resourceSubTab}
                onValueChange={(value) => {
                  setResourceSubTab(value as ResourceSubTab);
                  setRightPaneView("catalog");
                }}
                className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
              >
                <TabsList variant="line" className="h-8 w-full">
                  <TabsTrigger value="items" className="flex-1">
                    <FileIcon />
                    {t("resourceList")}
                  </TabsTrigger>
                  <TabsTrigger value="templates" className="flex-1">
                    <LayoutTemplateIcon />
                    {t("templateList")}
                  </TabsTrigger>
                </TabsList>

                <TabsContent
                  value="items"
                  className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
                >
                  <SectionHeading
                    title={t("resourceList")}
                    titleAs="h2"
                    description={t("itemsCount", { count: resources.length })}
                    descriptionPlacement="inline"
                    className="border-b-0 pb-0"
                  />
                  {resources.length ? (
                    <ScrollPane
                      className="flex-1"
                      innerClassName="flex flex-col gap-2"
                    >
                      {resources.map((resource) => {
                        const key = resourceKey(resource);
                        const mimeType = resourceMimeType(resource);
                        return (
                          <SelectableRowButton
                            key={key}
                            meta={mimeType || resource.uri}
                            onClick={() => {
                              setSelectedResourceKey(key);
                              setRightPaneView("catalog");
                            }}
                            selected={
                              rightPaneView === "catalog" &&
                              resourceSubTab === "items" &&
                              key === resourceKey(selectedResource || resource)
                            }
                            title={
                              resource.title || resource.name || resource.uri
                            }
                          />
                        );
                      })}
                    </ScrollPane>
                  ) : (
                    <PageEmpty
                      title={t("noResourcesFound")}
                      description={t("noResourcesFoundDescription")}
                      onRefresh={loadDetail}
                    />
                  )}
                </TabsContent>

                <TabsContent
                  value="templates"
                  className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
                >
                  <SectionHeading
                    title={t("templateList")}
                    titleAs="h2"
                    description={t("itemsCount", {
                      count: resourceTemplates.length,
                    })}
                    descriptionPlacement="inline"
                    className="border-b-0 pb-0"
                  />
                  {resourceTemplates.length ? (
                    <ScrollPane
                      className="flex-1"
                      innerClassName="flex flex-col gap-2"
                    >
                      {resourceTemplates.map((template) => {
                        const key = resourceTemplateKey(template);
                        const mimeType = resourceTemplateMimeType(template);
                        const uriTemplate = resourceTemplateUri(template);
                        return (
                          <SelectableRowButton
                            key={key}
                            meta={mimeType || uriTemplate}
                            onClick={() => {
                              setSelectedTemplateKey(key);
                              setRightPaneView("catalog");
                            }}
                            selected={
                              rightPaneView === "catalog" &&
                              resourceSubTab === "templates" &&
                              key ===
                                resourceTemplateKey(
                                  selectedTemplate || template,
                                )
                            }
                            title={
                              template.title || template.name || uriTemplate
                            }
                          />
                        );
                      })}
                    </ScrollPane>
                  ) : (
                    <PageEmpty
                      title={t("noTemplatesFound")}
                      description={t("noTemplatesFoundDescription")}
                      onRefresh={loadDetail}
                    />
                  )}
                </TabsContent>
              </Tabs>
            </TabsContent>

            <TabsContent
              value="prompts"
              className="mt-0 flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
            >
              <SectionHeading
                title={t("promptList")}
                titleAs="h2"
                description={t("itemsCount", { count: prompts.length })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {prompts.length ? (
                <ScrollPane
                  className="flex-1"
                  innerClassName="flex flex-col gap-2"
                >
                  {prompts.map((prompt) => {
                    const key = promptKey(prompt);
                    const argCount = promptArgCount(prompt);
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={
                          prompt.description ||
                          (argCount
                            ? t("paramCount", { count: argCount })
                            : t("noArgs"))
                        }
                        onClick={() => {
                          setSelectedPromptKey(key);
                          setRightPaneView("catalog");
                        }}
                        selected={
                          rightPaneView === "catalog" &&
                          key === promptKey(selectedPrompt || prompt)
                        }
                        title={prompt.title || prompt.name}
                      />
                    );
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noPromptsFound")}
                  description={t("noPromptsFoundDescription")}
                  onRefresh={loadDetail}
                />
              )}
            </TabsContent>
          </Tabs>
        </PanelCard>

        <PanelCard
          variant="plain"
          className="flex h-full min-h-0 flex-col gap-4 overflow-hidden"
        >
          <ServicePreviewHeader
            rightPaneView={rightPaneView}
            activeTab={activeTab}
            resourceSubTab={resourceSubTab}
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
              rightPaneView === "catalog" &&
              activeTab === "tools" &&
              selectedTool
                ? () =>
                    props.onRunTool(
                      selectedTool,
                      serializeToolArgs(toolArgs, toolArgsSchema),
                    )
                : undefined
            }
            runningTool={runningSelectedTool}
            onDetail={
              rightPaneView === "catalog" &&
              activeTab === "tools" &&
              selectedTool
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
                hint={
                  service.scope.type === "store"
                    ? t("store")
                    : `${t("agent")} ${service.scope.agent_id}`
                }
              />
              <MetricTile
                variant="compact"
                label={t("endpoint")}
                value={String(endpoint)}
                title={String(endpoint)}
                hint={
                  configArgs
                    ? `${transport} · ${configArgs}`
                    : t("transportSuffix", { transport })
                }
              />
              <MetricTile
                variant="compact"
                label={t("status")}
                value={String(service.state.readiness.status || t("unknown"))}
                title={String(service.state.readiness.status || t("unknown"))}
                hint={statusReport?.health || t("clickToManage")}
                active={statusDialogOpen}
                onClick={() => setStatusDialogOpen(true)}
              />
              <MetricTile
                variant="compact"
                label={t("catalog")}
                value={String(
                  tools.length +
                    resources.length +
                    resourceTemplates.length +
                    prompts.length,
                )}
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
              <PageError
                title={t("serviceDetailLoadFailed")}
                message={errorMessage}
                onRefresh={loadDetail}
              />
            </ScrollPane>
          ) : rightPaneView === "catalog" &&
            activeTab === "tools" &&
            selectedTool ? (
            <div className="grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(0,1fr)_minmax(12rem,22rem)] grid-rows-1 gap-6 overflow-hidden">
              <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
                <ToolDetailDocHeader tool={selectedTool} />
                <ScrollPane className="min-h-0 flex-1">
                  <ToolDetailDocBody
                    tool={selectedTool}
                    toolArgs={toolArgs}
                    onToolArgChange={setToolArg}
                  />
                </ScrollPane>
              </div>
              <ToolPlaygroundAside
                tool={selectedTool}
                instanceId={service.instance_id}
                toolArgs={toolArgs}
                toolArgsSchema={toolArgsSchema}
                running={runningSelectedTool}
                onRun={() =>
                  props.onRunTool(
                    selectedTool,
                    serializeToolArgs(toolArgs, toolArgsSchema),
                  )
                }
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
                <PageEmpty
                  title={t("noToolSelected")}
                  description={t("serviceToolDetailsWillAppear")}
                  onRefresh={loadDetail}
                />
              ) : activeTab === "resources" ? (
                resourceSubTab === "templates" ? (
                  selectedTemplate ? (
                    <ServiceResourceTemplateDetailPane
                      template={selectedTemplate}
                    />
                  ) : (
                    <PageEmpty
                      title={t("noTemplateSelected")}
                      description={t("noTemplateSelectedDescription")}
                      onRefresh={loadDetail}
                    />
                  )
                ) : selectedResource ? (
                  <ServiceResourceDetailPane
                    resource={selectedResource}
                    instanceId={service.instance_id}
                  />
                ) : (
                  <PageEmpty
                    title={t("noResourceSelected")}
                    description={t("noResourceSelectedDescription")}
                    onRefresh={loadDetail}
                  />
                )
              ) : selectedPrompt ? (
                <ServicePromptDetailPane prompt={selectedPrompt} />
              ) : (
                <PageEmpty
                  title={t("noPromptSelected")}
                  description={t("noPromptSelectedDescription")}
                  onRefresh={loadDetail}
                />
              )}
            </ScrollPane>
          )}
        </PanelCard>
      </TwoPanePage>

      <ServiceStatusActionsDialog
        busy={props.busy}
        open={statusDialogOpen}
        service={service}
        serviceState={service.state}
        onConnect={props.onConnect}
        onDelete={props.onDelete}
        onDisconnect={props.onDisconnect}
        onOpenChange={setStatusDialogOpen}
        onRestart={props.onRestart}
      />
    </>
  );
}
