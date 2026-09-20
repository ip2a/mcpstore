import { useEffect, useState, type ReactNode } from "react";
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
import { ServiceAuthConnectionFields } from "@/features/services/service-auth-panel";
import { ServiceConnectionButton } from "@/features/services/service-connection-button";
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
  type ToolInfo,
} from "@/lib/api";
import { formatServiceLaunchLine } from "@/lib/service-info";
import { useI18n } from "@/lib/i18n-context";
import { cn } from "@/lib/utils";
import { OverrideEditor } from "@/features/overrides/override-editor";

export type CatalogTab = "tools" | "resources" | "prompts";
export type ResourceSubTab = "items" | "templates";
export type RightPaneView = "service" | "catalog";

export function resourceMimeType(resource: ResourceInfo) {
  return String(resource.mimeType || "").trim();
}

export function promptArgCount(prompt: PromptInfo) {
  const args = prompt.arguments;
  if (!args) return 0;
  if (Array.isArray(args)) return args.length;
  if (
    typeof args === "object" &&
    args !== null &&
    Array.isArray((args as { properties?: unknown[] }).properties)
  )
    return (args as { properties: unknown[] }).properties.length;
  return 0;
}

export function ServicePreviewHeader({
  rightPaneView,
  activeTab,
  resourceSubTab,
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
  onRefresh,
  onConnect,
  onDisconnect,
  busy,
  onRun,
  runningTool,
  onDetail,
}: {
  rightPaneView: RightPaneView;
  activeTab: CatalogTab;
  resourceSubTab: ResourceSubTab;
  loading: boolean;
  service: ServiceInstance;
  selectedTool: ToolInfo | null;
  selectedResource: ResourceInfo | null;
  selectedTemplate: ResourceTemplateInfo | null;
  selectedPrompt: PromptInfo | null;
  toolCount: number;
  resourceCount: number;
  templateCount: number;
  promptCount: number;
  onRefresh: () => void;
  onConnect: () => void;
  onDisconnect: () => void;
  busy: string | null;
  onRun?: () => void;
  runningTool?: boolean;
  onDetail?: () => void;
}) {
  const { t } = useI18n();
  const title =
    rightPaneView === "service"
      ? service.service_name
      : activeTab === "tools"
        ? selectedTool?.name || t("toolsAvailable", { count: toolCount })
        : activeTab === "resources"
          ? resourceSubTab === "templates"
            ? selectedTemplate?.title ||
              selectedTemplate?.name ||
              (selectedTemplate ? resourceTemplateUri(selectedTemplate) : "") ||
              t("templatesAvailable", { count: templateCount })
            : selectedResource?.title ||
              selectedResource?.name ||
              selectedResource?.uri ||
              t("resourcesAvailable", { count: resourceCount })
          : selectedPrompt?.title ||
            selectedPrompt?.name ||
            t("promptsAvailable", { count: promptCount });

  const copyPayload =
    rightPaneView === "service"
      ? service
      : activeTab === "tools"
        ? selectedTool
        : activeTab === "resources"
          ? resourceSubTab === "templates"
            ? selectedTemplate
            : selectedResource
          : selectedPrompt;

  async function onCopy() {
    if (!copyPayload) return;
    await navigator.clipboard.writeText(JSON.stringify(copyPayload, null, 2));
    toast.success(t("copied"));
  }

  async function onReadResource() {
    if (!selectedResource) return;
    try {
      const result = await readInstanceResource(
        service,
        selectedResource.uri,
      );
      await navigator.clipboard.writeText(JSON.stringify(result, null, 2));
      toast.success(t("resourceContentCopied"));
    } catch (err) {
      const message =
        err instanceof Error ? err.message : t("readResourceFailed");
      toast.error(message);
    }
  }

  const showToolHeader =
    rightPaneView === "catalog" &&
    activeTab === "tools" &&
    Boolean(selectedTool);

  return (
    <div className="flex flex-wrap items-center gap-3 border-b pb-2">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        {showToolHeader ? (
          <h2
            className="shrink-0 truncate font-mono text-sm font-medium"
            title={selectedTool!.name}
          >
            {selectedTool!.name}
          </h2>
        ) : (
          <div className="flex min-w-0 flex-col gap-1">
            <strong
              className="truncate font-mono text-sm font-medium"
              title={title}
            >
              {title}
            </strong>
          </div>
        )}
      </div>
      <div className="flex shrink-0 flex-wrap justify-end gap-2">
        {rightPaneView === "service" ? (
          <ServiceConnectionButton
            busy={busy}
            instanceId={service.instance_id}
            state={service.state}
            onConnect={onConnect}
            onDisconnect={onDisconnect}
          />
        ) : null}
        {rightPaneView === "catalog" && activeTab === "tools" && onRun ? (
          <Button size="sm" onClick={onRun} disabled={runningTool}>
            {runningTool ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <WrenchIcon data-icon="inline-start" />
            )}
            {runningTool ? t("executing") : t("run")}
          </Button>
        ) : null}
        {rightPaneView === "catalog" && activeTab === "tools" && onDetail ? (
          <Button size="sm" variant="outline" onClick={onDetail}>
            <EyeIcon data-icon="inline-start" />
            {t("details")}
          </Button>
        ) : null}
        {rightPaneView === "catalog" &&
        activeTab === "resources" &&
        resourceSubTab === "items" &&
        selectedResource ? (
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
        <Button
          size="sm"
          variant="outline"
          onClick={onRefresh}
          disabled={loading}
        >
          <RefreshCwIcon data-icon="inline-start" />
          {t("refresh")}
        </Button>
      </div>
    </div>
  );
}

function ServiceDetailField({
  label,
  value,
  mono = true,
  span = false,
}: {
  label: string;
  value: ReactNode;
  mono?: boolean;
  span?: boolean;
}) {
  return (
    <div className={cn("grid gap-1", span && "@min-[32rem]:col-span-2")}>
      <dt className="font-mono text-xs uppercase text-muted-foreground">
        {label}
      </dt>
      <dd className={cn("break-words text-sm", mono && "font-mono")}>
        {value}
      </dd>
    </div>
  );
}

export function ServiceOverviewPane({
  service,
  description,
  transport,
  launchLine,
  configArgs,
  endpoint,
}: {
  service: ServiceInstance;
  description: string;
  transport: string;
  launchLine: string;
  configArgs: string | null;
  endpoint: string;
}) {
  const { t } = useI18n();
  const capabilities = service.mcp?.capabilities;
  const capabilityLabels = capabilities
    ? [
        capabilities.tools && t("tools"),
        capabilities.resources && t("resources"),
        capabilities.resourcesSubscribe && t("resourceSubscriptions"),
        capabilities.prompts && t("prompts"),
        capabilities.completions && t("completions"),
        capabilities.tasks && t("tasks"),
      ].filter((label): label is string => Boolean(label))
    : [];
  const scopeLabel =
    service.scope.type === "store"
      ? t("store")
      : `${t("agent")} ${service.scope.agent_id}`;

  return (
    <div className="@container flex min-w-0 flex-col gap-5">
      <section className="flex flex-col gap-3">
        <SectionHeading
          title={t("serviceDetail")}
          titleAs="h2"
          className="border-b-0 pb-0"
        />
        <dl className="grid gap-3 @min-[32rem]:grid-cols-2">
          <ServiceDetailField label={t("name")} value={service.service_name} />
          <ServiceDetailField label={t("agentScope")} value={scopeLabel} />
          <ServiceDetailField
            label="Instance ID"
            value={service.instance_id}
            span
          />
          {description ? (
            <ServiceDetailField
              label={t("description")}
              value={description}
              mono={false}
              span
            />
          ) : null}
        </dl>
      </section>

      <section className="flex flex-col gap-3 border-t border-border/50 pt-5">
        <SectionHeading
          title={t("connection")}
          titleAs="h2"
          className="border-b-0 pb-0"
        />
        <ServiceAuthConnectionFields
          service={service}
          render={({ statusField, actions }) => (
            <>
              <dl className="grid gap-3 @min-[32rem]:grid-cols-2">
                <ServiceDetailField
                  label={t("transport")}
                  value={
                    configArgs ? `${transport} · ${configArgs}` : transport
                  }
                />
                {statusField}
                <ServiceDetailField label={t("endpoint")} value={endpoint} span />
                <ServiceDetailField label={t("launch")} value={launchLine} span />
              </dl>
              {actions}
            </>
          )}
        />
      </section>

      {service.mcp ? (
        <section className="flex flex-col gap-3 border-t border-border/50 pt-5">
          <SectionHeading
            title={t("mcpServer")}
            titleAs="h2"
            className="border-b-0 pb-0"
          />
          <dl className="grid gap-3 @min-[32rem]:grid-cols-2">
            <ServiceDetailField
              label={t("serverImplementation")}
              value={`${service.mcp.serverInfo.title || service.mcp.serverInfo.name} · ${service.mcp.serverInfo.version}`}
            />
            <ServiceDetailField
              label={t("protocolVersion")}
              value={service.mcp.protocolVersion}
            />
            {capabilityLabels.length ? (
              <div className="@min-[32rem]:col-span-2">
                <dt className="font-mono text-xs uppercase text-muted-foreground">
                  {t("capabilities")}
                </dt>
                <dd className="mt-1 flex flex-wrap gap-2">
                  {capabilityLabels.map((label) => (
                    <Badge key={label} variant="outline">
                      {label}
                    </Badge>
                  ))}
                </dd>
              </div>
            ) : null}
            {service.mcp.instructions ? (
              <ServiceDetailField
                label={t("serverInstructions")}
                value={
                  <span className="whitespace-pre-wrap font-sans text-sm">
                    {service.mcp.instructions}
                  </span>
                }
                mono={false}
                span
              />
            ) : null}
          </dl>
        </section>
      ) : null}
    </div>
  );
}

export function ServiceResourceDetailPane({
  resource,
  service,
}: {
  resource: ResourceInfo;
  service: ServiceInstance;
}) {
  const { t } = useI18n();
  const [content, setContent] = useState<unknown>(null);
  const [reading, setReading] = useState(false);
  const mimeType = resourceMimeType(resource);

  async function onRead() {
    setReading(true);
    try {
      const result = await readInstanceResource(service, resource.uri);
      setContent(result);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : t("readResourceFailed");
      toast.error(message);
    } finally {
      setReading(false);
    }
  }

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading
          title={t("resource")}
          titleAs="h2"
          className="border-b-0 pb-3"
        />
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
          <Button
            size="sm"
            variant="outline"
            onClick={onRead}
            disabled={reading}
          >
            <FileIcon data-icon="inline-start" />
            {reading ? t("reading") : t("readResource")}
          </Button>
        </div>
      </section>
      {content ? (
        <section className="pb-2">
          <SectionHeading
            title={t("content")}
            titleAs="h2"
            className="border-b-0 pb-3"
          />
          <JsonBlock value={content} />
        </section>
      ) : null}
      <OverrideEditor kind="resource" serviceName={service.service_name} componentKey={resource.uri} scope={service.scope} />
    </div>
  );
}

export function ServiceResourceTemplateDetailPane({
  template,
  service,
}: {
  template: ResourceTemplateInfo;
  service: ServiceInstance;
}) {
  const { t } = useI18n();
  const uriTemplate = resourceTemplateUri(template);
  const mimeType = resourceTemplateMimeType(template);
  const annotations = getResourceTemplateAnnotations(template);
  const meta = getResourceTemplateMeta(template);

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading
          title={t("resourceTemplate")}
          titleAs="h2"
          className="border-b-0 pb-3"
        />
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
          <SectionHeading
            title={t("annotations")}
            titleAs="h2"
            className="border-b-0 pb-3"
          />
          <JsonBlock value={annotations} />
        </section>
      ) : null}
      {hasResourceTemplateMeta(template) ? (
        <section className="pb-2">
          <SectionHeading
            title={t("meta")}
            titleAs="h2"
            className="border-b-0 pb-3"
          />
          <JsonBlock value={meta} />
        </section>
      ) : null}
      <OverrideEditor kind="resource_template" serviceName={service.service_name} componentKey={uriTemplate} scope={service.scope} />
    </div>
  );
}

export function ServicePromptDetailPane({ prompt, service }: { prompt: PromptInfo; service: ServiceInstance }) {
  const { t } = useI18n();
  const argCount = promptArgCount(prompt);

  return (
    <div className="flex min-w-0 flex-col gap-4">
      <section className="border-b pb-4">
        <SectionHeading
          title={t("promptLabel")}
          titleAs="h2"
          className="border-b-0 pb-3"
        />
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
      <OverrideEditor kind="prompt" serviceName={service.service_name} componentKey={prompt.name} scope={service.scope} />
      <section className="pb-2">
        <SectionHeading
          title={t("arguments")}
          titleAs="h2"
          badge={argCount || undefined}
          className="border-b-0 pb-3"
        />
        {prompt.arguments ? (
          <JsonBlock value={prompt.arguments} />
        ) : (
          <p className="rounded-lg border border-dashed bg-muted/10 px-3 py-4 text-sm text-muted-foreground">
            {t("noPromptArguments")}
          </p>
        )}
      </section>
    </div>
  );
}
