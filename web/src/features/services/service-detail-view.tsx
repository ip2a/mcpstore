import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";

import { PageEmpty, PageError } from "@/components/shared/page-states";
import { PanelCard } from "@/components/shared/panel-card";
import { ScrollPane } from "@/components/shared/scroll-pane";
import { SectionHeading } from "@/components/shared/section-heading";
import { SelectableRowButton } from "@/components/shared/selectable-row-button";
import {
  ToolDetailDocBody,
  ToolDetailDocHeader,
  ToolPlaygroundAside,
} from "@/components/shared/tool-detail-playground";
import { toolDetailPlaygroundGrid } from "@/components/shared/tool-detail-section-layout";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  useServiceAuthQuery,
  useServiceDetailQuery,
  useServicePromptsQuery,
  useServiceResourceTemplatesQuery,
  useServiceResourcesQuery,
  useServiceStatusQuery,
} from "@/features/services/queries";
import { useToolArgsForm } from "@/features/tools/use-tool-args-form";
import { serializeToolArgs } from "@/lib/tool-args";
import { getOriginalToolName, getToolSchema, toolKey } from "@/lib/tool-info";
import {
  resourceTemplateKey,
  resourceTemplateMimeType,
  resourceTemplateUri,
} from "@/lib/resource-template-info";
import {
  type PromptInfo,
  type ResourceInfo,
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
  ServicePreviewActions,
  ServicePromptDetailPane,
  ServiceResourceDetailPane,
  ServiceResourceTemplateDetailPane,
  type ResourceSubTab,
  type ServiceDetailTab,
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
  const [activeTab, setActiveTab] = useState<ServiceDetailTab>("service");
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
  const detailQuery = useServiceDetailQuery(props.service);
  const authQuery = useServiceAuthQuery(props.service);
  const statusQuery = useServiceStatusQuery(props.service);
  const resourcesQuery = useServiceResourcesQuery(props.service);
  const resourceTemplatesQuery = useServiceResourceTemplatesQuery(props.service);
  const promptsQuery = useServicePromptsQuery(props.service);
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
    if (resourceSubTab === "items" && !resources.length && resourceTemplates.length) {
      setResourceSubTab("templates");
    } else if (
      resourceSubTab === "templates" &&
      !resourceTemplates.length &&
      resources.length
    ) {
      setResourceSubTab("items");
    }
  }, [resourceSubTab, resourceTemplates.length, resources.length]);

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
      ]);
      if (nextDetail.error) throw nextDetail.error;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : t("serviceDetailLoadFailed");
      if (!detailQuery.error) setDetailError(message);
      toast.error(message);
    }
  }

  async function loadCatalog() {
    if (activeTab === "resources") {
      await Promise.all([
        resourcesQuery.refetch(),
        resourceTemplatesQuery.refetch(),
      ]);
      return;
    }
    if (activeTab === "prompts") await promptsQuery.refetch();
  }

  function refreshCurrentView() {
    if (activeTab === "service") {
      return Promise.all([loadDetail(), authQuery.refetch()]);
    }
    return activeTab === "tools" ? loadDetail() : loadCatalog();
  }

  useEffect(() => {
    setActiveTab("service");
    setResourceSubTab("items");
  }, [props.service.instance_id]);

  useEffect(() => {
    void loadDetail();
  }, [props.refreshToken, props.service.instance_id]);

  useEffect(() => {
    if (activeTab === "prompts" || activeTab === "resources") void loadCatalog();
  }, [activeTab, props.service.instance_id]);

  const catalogSplitClassName =
    "mt-0 grid min-h-0 min-w-0 flex-1 grid-cols-[minmax(220px,0.38fr)_minmax(0,1fr)] grid-rows-1 gap-4 overflow-hidden [&>*]:min-h-0 [&>*]:min-w-0";

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
        ? () =>
            props.onRunTool(
              selectedTool,
              serializeToolArgs(toolArgs, toolArgsSchema),
            )
        : undefined,
    runningTool: runningSelectedTool,
    onDetail:
      activeTab === "tools" && selectedTool
        ? () => props.onToolDetail(selectedTool, service, statusReport)
        : undefined,
  };

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
    );
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
          <div className={toolDetailPlaygroundGrid}>
            <div className="flex min-h-0 min-w-0 flex-col overflow-hidden">
              <ToolDetailDocHeader tool={selectedTool} />
              <ScrollPane className="min-h-0 flex-1">
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
                <ServiceResourceDetailPane
                  resource={selectedResource}
                  service={service}
                />
              ) : (
                <PageEmpty
                  title={t("noResourceSelected")}
                  description={t("noResourceSelectedDescription")}
                  onRefresh={refreshCurrentView}
                />
              )
            ) : selectedPrompt ? (
              <ServicePromptDetailPane
                prompt={selectedPrompt}
                service={service}
              />
            ) : (
              <PageEmpty
                title={t("noPromptSelected")}
                onRefresh={refreshCurrentView}
              />
            )}
          </ScrollPane>
        )}
      </div>
    );
  }

  return (
    <>
      <PanelCard className="@container flex h-full min-h-0 flex-1 flex-col overflow-hidden">
        <section className="flex shrink-0 flex-col gap-3 border-b pb-4">
          <div className="min-w-0">
            <p className="font-mono text-xs uppercase text-muted-foreground">
              {t("service")}
            </p>
            <button
              type="button"
              onClick={() => setActiveTab("service")}
              className={cn(
                "mt-1 block max-w-full cursor-pointer truncate border-0 bg-transparent p-0 text-left text-lg font-semibold underline-offset-4 outline-none transition-opacity",
                "hover:underline active:opacity-70",
                activeTab === "service" && "underline",
              )}
              title={service.service_name}
            >
              {service.service_name}
            </button>
            <p
              className="mt-1 truncate font-mono text-xs text-muted-foreground"
              title={transport}
            >
              {transport}
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
          onValueChange={(value) => setActiveTab(value as ServiceDetailTab)}
          className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden"
        >
          <div className="flex min-w-0 shrink-0 items-center gap-3 pb-2">
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
            <div className="flex min-h-0 flex-col gap-3 overflow-hidden border-r border-border/50 pr-4">
              <SectionHeading
                title={t("toolList")}
                titleAs="h2"
                description={t("itemsCount", { count: tools.length })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {tools.length ? (
                <ScrollPane
                  className="flex-1"
                  innerClassName="flex flex-col gap-2"
                >
                  {tools.map((tool) => {
                    const key = toolKey(service.instance_id, tool);
                    const schema = getToolSchema(tool) as {
                      properties?: Record<string, unknown>;
                    };
                    const paramCount = Object.keys(
                      schema.properties || {},
                    ).length;
                    return (
                      <SelectableRowButton
                        key={key}
                        meta={t("paramCount", { count: paramCount })}
                        onClick={() => setSelectedToolKey(key)}
                        selected={
                          key ===
                          toolKey(service.instance_id, selectedTool || tool)
                        }
                        title={tool.name}
                      />
                    );
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noToolsFound")}
                  description={t("noToolsFoundDescription")}
                  onRefresh={refreshCurrentView}
                />
              )}
            </div>
            {renderCatalogDetail()}
          </TabsContent>

          <TabsContent value="prompts" className={catalogSplitClassName}>
            <div className="flex min-h-0 flex-col gap-3 overflow-hidden border-r border-border/50 pr-4">
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
                        onClick={() => setSelectedPromptKey(key)}
                        selected={key === promptKey(selectedPrompt || prompt)}
                        title={prompt.title || prompt.name}
                      />
                    );
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noPromptsFound")}
                  onRefresh={refreshCurrentView}
                />
              )}
            </div>
            {renderCatalogDetail()}
          </TabsContent>

          <TabsContent value="resources" className={catalogSplitClassName}>
            <div className="flex min-h-0 flex-col gap-3 overflow-hidden border-r border-border/50 pr-4">
              <SectionHeading
                title={t("resourceList")}
                titleAs="h2"
                description={t("itemsCount", {
                  count: resources.length + resourceTemplates.length,
                })}
                descriptionPlacement="inline"
                className="border-b-0 pb-0"
              />
              {resources.length || resourceTemplates.length ? (
                <ScrollPane
                  className="flex-1"
                  innerClassName="flex flex-col gap-2"
                >
                  {resources.map((resource) => {
                    const key = resourceKey(resource);
                    const mimeType = resourceMimeType(resource);
                    return (
                      <SelectableRowButton
                        key={`resource:${key}`}
                        meta={mimeType || resource.uri}
                        onClick={() => {
                          setResourceSubTab("items");
                          setSelectedResourceKey(key);
                        }}
                        selected={
                          resourceSubTab === "items" &&
                          key === resourceKey(selectedResource || resource)
                        }
                        title={
                          resource.title || resource.name || resource.uri
                        }
                      />
                    );
                  })}
                  {resourceTemplates.map((template) => {
                    const key = resourceTemplateKey(template);
                    const mimeType = resourceTemplateMimeType(template);
                    const uriTemplate = resourceTemplateUri(template);
                    return (
                      <SelectableRowButton
                        key={`template:${key}`}
                        meta={mimeType || uriTemplate}
                        onClick={() => {
                          setResourceSubTab("templates");
                          setSelectedTemplateKey(key);
                        }}
                        selected={
                          resourceSubTab === "templates" &&
                          key ===
                            resourceTemplateKey(selectedTemplate || template)
                        }
                        title={
                          template.title || template.name || uriTemplate
                        }
                        trailing={
                          <Badge variant="outline">{t("templates")}</Badge>
                        }
                      />
                    );
                  })}
                </ScrollPane>
              ) : (
                <PageEmpty
                  title={t("noResourcesFound")}
                  description={t("noResourcesFoundDescription")}
                  onRefresh={refreshCurrentView}
                />
              )}
            </div>
            {renderCatalogDetail()}
          </TabsContent>
        </Tabs>
      </PanelCard>
    </>
  );
}
