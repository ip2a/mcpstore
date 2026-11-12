import { useMemo, useState } from "react"
import { PlayIcon } from "lucide-react"
import { toast } from "sonner"

import { ServiceStatusBadge } from "@/components/shared/service-status-badge"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import { ToolSchemaEmptyHint } from "@/components/shared/tool-schema-empty-hint"
import { toolDetailPlaygroundAside } from "@/components/shared/tool-detail-section-layout"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockBody,
  CodeBlockCopyButton,
  CodeBlockFooter,
  CodeBlockHeader,
  CodeBlockJson,
  CodeBlockMethod,
  CodeBlockString,
  CodeBlockTitle,
} from "@/components/ui/code-block"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { TypographyH2, TypographyInlineCode, TypographyLead } from "@/components/ui/typography"
import { OverrideEditor } from "@/features/overrides/override-editor"
import {
  getInstancePrompt,
  type PromptInfo,
  type ScopeRef,
  type ServiceInstance,
  type ServiceState,
} from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"
import { parsePromptArgs, type PromptArgDef } from "@/lib/prompt-info"
import { hasJsonContent } from "@/lib/tool-info"
import { cn } from "@/lib/utils"
import { JsonBlock } from "@/components/shared/json-block"

export function buildPromptCliCommand(input: {
  instanceId: string
  promptName: string
  args: Record<string, unknown>
}) {
  const argsJson = JSON.stringify(input.args)
  return [
    "mcpstore prompts get \\",
    `  --instance "${input.instanceId}" \\`,
    `  --prompt "${input.promptName}" \\`,
    `  --args '${argsJson}'`,
  ].join("\n")
}

export function PromptDetailDocHeader({
  prompt,
  serviceState,
  className,
}: {
  prompt: PromptInfo
  serviceState?: ServiceState | null
  className?: string
}) {
  const title = prompt.title?.trim() || prompt.name

  return (
    <header className={cn("shrink-0 space-y-3 border-b pb-4", className)}>
      <div className="flex min-w-0 items-center justify-between gap-3">
        <TypographyH2 className="min-w-0 border-b-0 pb-0 break-words font-mono text-xl">
          {title}
        </TypographyH2>
        {serviceState ? <ServiceStatusBadge state={serviceState} /> : null}
      </div>
      <ToolDescriptionBlock
        description={prompt.description}
        showLabel={false}
        omitStructuredSections
        className="text-foreground"
      />
    </header>
  )
}

export function PromptDetailDocBody({
  prompt,
  promptArgs,
  onPromptArgChange,
  serviceName,
  scope,
  className,
}: {
  prompt: PromptInfo
  promptArgs: Record<string, unknown>
  onPromptArgChange: (name: string, value: unknown) => void
  serviceName?: string
  scope?: ScopeRef
  className?: string
}) {
  const { t } = useI18n()
  const defs = parsePromptArgs(prompt)
  const meta = prompt._meta ?? null
  const fields: Array<{ label: string; value: string; mono?: boolean }> = [
    { label: t("name"), value: prompt.name, mono: true },
  ]
  if (prompt.title?.trim() && prompt.title.trim() !== prompt.name) {
    fields.push({ label: t("titleLabel"), value: prompt.title.trim() })
  }

  return (
    <div className={cn("min-w-0 space-y-6 pb-1 pt-4", className)}>
      <section>
        <TypographyH2>{t("promptLabel")}</TypographyH2>
        <dl className="mt-3 grid gap-4">
          {fields.map((field) => (
            <div key={field.label} className="grid gap-1 border-b pb-4 last:border-b-0 last:pb-0">
              <dt className="font-mono text-xs uppercase text-muted-foreground">{field.label}</dt>
              <dd className={cn("break-all text-sm", field.mono && "font-mono")}>{field.value}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section>
        <TypographyH2>{t("queryParameters")}</TypographyH2>
        <PromptArgDocList
          defs={defs}
          values={promptArgs}
          onChange={onPromptArgChange}
          emptyMessage={t("noPromptArguments")}
        />
      </section>

      {hasJsonContent(meta) ? (
        <section>
          <TypographyH2>{t("meta")}</TypographyH2>
          <div className="mt-3">
            <JsonBlock value={meta} />
          </div>
        </section>
      ) : null}

      {serviceName && scope ? (
        <OverrideEditor
          kind="prompt"
          serviceName={serviceName}
          componentKey={prompt.name}
          scope={scope}
        />
      ) : null}
    </div>
  )
}

function PromptArgDocList({
  defs,
  values,
  onChange,
  emptyMessage,
}: {
  defs: PromptArgDef[]
  values: Record<string, unknown>
  onChange: (name: string, value: unknown) => void
  emptyMessage: string
}) {
  const { t } = useI18n()

  if (!defs.length) {
    return <ToolSchemaEmptyHint>{emptyMessage}</ToolSchemaEmptyHint>
  }

  return (
    <div className="@container flex flex-col">
      {defs.map((def, index) => {
        const isLast = index === defs.length - 1
        return (
          <article
            key={def.name}
            className={cn("grid gap-3 border-b py-4", isLast && "border-b-0 pb-0")}
          >
            <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <TypographyInlineCode className="break-all text-sm font-semibold">
                {def.name}
              </TypographyInlineCode>
              <span className="font-mono text-xs text-muted-foreground">string</span>
              {def.required ? (
                <span className="text-xs font-medium text-foreground">{t("required")}</span>
              ) : (
                <span className="text-xs text-muted-foreground">{t("optional")}</span>
              )}
            </div>
            {def.description ? (
              <p className="text-sm leading-relaxed text-muted-foreground">{def.description}</p>
            ) : null}
            <div className="grid gap-3 @min-[32rem]:grid-cols-[minmax(0,3fr)_minmax(0,7fr)] @min-[32rem]:items-start">
              <span aria-hidden="true" />
              <Input
                id={`prompt-param-${def.name}`}
                value={String(values[def.name] ?? "")}
                onChange={(event) => onChange(def.name, event.target.value)}
                className="bg-muted/50"
              />
            </div>
          </article>
        )
      })}
    </div>
  )
}

export function PromptPlaygroundAside({
  prompt,
  service,
  promptArgs,
  onGet,
  getting,
  content,
  className,
}: {
  prompt: PromptInfo
  service: ServiceInstance
  promptArgs: Record<string, unknown>
  onGet?: () => void
  getting?: boolean
  content?: unknown
  className?: string
}) {
  const { t } = useI18n()
  const [localContent, setLocalContent] = useState<unknown>(null)
  const [localGetting, setLocalGetting] = useState(false)

  const controlled = onGet !== undefined
  const isGetting = controlled ? Boolean(getting) : localGetting
  const displayContent = controlled ? content : localContent

  const cliCommand = useMemo(
    () =>
      buildPromptCliCommand({
        instanceId: service.instance_id,
        promptName: prompt.name,
        args: promptArgs,
      }),
    [prompt.name, promptArgs, service.instance_id],
  )

  const requestArgsText = useMemo(() => JSON.stringify(promptArgs, null, 2), [promptArgs])

  const responseText = useMemo(
    () =>
      displayContent == null
        ? JSON.stringify({ description: "...", messages: [] }, null, 2)
        : JSON.stringify(displayContent, null, 2),
    [displayContent],
  )

  async function requestGet() {
    if (controlled) {
      onGet?.()
      return
    }
    setLocalGetting(true)
    try {
      const result = await getInstancePrompt(service, prompt.name, promptArgs)
      setLocalContent(result)
    } catch (err) {
      const message = err instanceof Error ? err.message : t("getPromptFailed")
      toast.error(message)
    } finally {
      setLocalGetting(false)
    }
  }

  return (
    <aside
      className={cn(
        toolDetailPlaygroundAside,
        "h-full min-h-0 min-w-0 w-full flex-col gap-4",
        className,
      )}
    >
      <CodeBlock variant="request" className="flex shrink-0 flex-col">
        <CodeBlockHeader variant="request">
          <CodeBlockTitle>
            <CodeBlockMethod className="bg-sky-500/15 text-sky-300">GET</CodeBlockMethod>
            <span className="truncate text-zinc-300">{prompt.name}</span>
          </CodeBlockTitle>
          <CodeBlockActions>
            <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">
              {t("cliRequestLabel")}
            </span>
            <CodeBlockCopyButton
              value={cliCommand}
              className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50"
            />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="request" maxHeight="11rem">
          <CodeBlockString lines={cliCommand.split("\n")} />
        </CodeBlockBody>
        <CodeBlockFooter variant="request" className="justify-end">
          <Button
            size="sm"
            className="h-8 bg-zinc-100 text-zinc-900 hover:bg-white"
            onClick={() => void requestGet()}
            disabled={isGetting}
          >
            {isGetting ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <PlayIcon data-icon="inline-start" />
            )}
            {isGetting ? t("executing") : t("testRequest")}
          </Button>
        </CodeBlockFooter>
      </CodeBlock>

      <CodeBlock variant="response" className="flex min-h-0 flex-1 flex-col">
        <CodeBlockHeader variant="response" className="shrink-0">
          <CodeBlockTitle>
            <span className="font-semibold text-foreground">
              {displayContent == null ? t("dynamicValue") : "200"}
            </span>
          </CodeBlockTitle>
          <CodeBlockActions>
            <CodeBlockCopyButton
              value={displayContent == null ? requestArgsText : responseText}
            />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="response" fill nativeScroll>
          <CodeBlockJson text={displayContent == null ? requestArgsText : responseText} />
        </CodeBlockBody>
        <CodeBlockFooter variant="response" className="shrink-0">
          <TypographyLead className="m-0 text-xs text-muted-foreground">
            {displayContent == null ? t("requestPreview") : t("responseOk")}
          </TypographyLead>
        </CodeBlockFooter>
      </CodeBlock>
    </aside>
  )
}
