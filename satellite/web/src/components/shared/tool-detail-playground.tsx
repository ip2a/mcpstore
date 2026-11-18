import { useMemo } from "react"

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
import { ToolParameterDocList } from "@/components/shared/tool-parameter-doc-list"
import { ToolDescriptionBlock } from "@/components/shared/tool-description-block"
import {
  ToolAnnotationsSection,
  ToolMetaSection,
} from "@/components/shared/tool-capability-sections"
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion"
import { TypographyH2, TypographyLead } from "@/components/ui/typography"
import { useI18n } from "@/lib/i18n-context"
import type { ScopeRef, ToolInfo } from "@/lib/api"
import { InlineOverrideField } from "@/features/overrides/inline-override-field"
import { OverrideEditor } from "@/features/overrides/override-editor"
import { useToolInlineOverride } from "@/features/overrides/use-tool-inline-override"
import { buildToolCliCommand } from "@/lib/tool-schema-preview"
import { serializeToolArgs, type ToolSchema } from "@/lib/tool-args"
import {
  getOriginalToolName,
  getToolOutputSchema,
  getToolSchema,
  extractToolDescriptionDocs,
} from "@/lib/tool-info"
import { toolDetailPlaygroundAside } from "@/components/shared/tool-detail-section-layout"
import { cn } from "@/lib/utils"

type SchemaField = Record<string, unknown>
type JsonSchema = { properties?: Record<string, SchemaField>; required?: string[] }

function schemaFields(schema: JsonSchema) {
  return Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))
}

function useToolPlaygroundData({
  tool,
  instanceId,
  toolArgs,
  toolArgsSchema,
}: {
  tool: ToolInfo
  instanceId: string
  toolArgs: Record<string, unknown>
  toolArgsSchema: ToolSchema
}) {
  const inputSchema = getToolSchema(tool) as JsonSchema
  const outputSchema = getToolOutputSchema(tool) as JsonSchema
  const inputParams = schemaFields(inputSchema)
  const outputParams = schemaFields(outputSchema)
  const hasOutputSchema =
    outputParams.length > 0 || Object.keys(outputSchema.properties || {}).length > 0

  const cliCommand = useMemo(
    () =>
      buildToolCliCommand({
        instanceId,
        toolName: tool.name,
        args: toolArgs,
        toolArgsSchema,
      }),
    [instanceId, tool.name, toolArgs, toolArgsSchema],
  )

  const requestArgsText = useMemo(
    () => JSON.stringify(serializeToolArgs(toolArgs, toolArgsSchema), null, 2),
    [toolArgs, toolArgsSchema],
  )

  return {
    cliCommand,
    hasOutputSchema,
    inputParams,
    inputSchema,
    outputParams,
    outputSchema,
    requestArgsText,
  }
}

export function ToolDetailDocHeader({
  tool,
  className,
  serviceName,
  scope,
  onDisplayNameApplied,
}: {
  tool: ToolInfo
  className?: string
  serviceName?: string
  scope?: ScopeRef
  onDisplayNameApplied?: (projectedName: string) => void
}) {
  const { t } = useI18n()
  const originalToolName = getOriginalToolName(tool)
  const override = useToolInlineOverride(serviceName, originalToolName, scope, {
    onDisplayNameApplied,
  })
  const description = tool.description?.trim() || t("noDescription")

  return (
    <header className={cn("shrink-0 space-y-3 border-b pb-4", className)}>
      <TypographyH2 className="min-w-0 border-b-0 pb-0 break-words font-mono text-xl">
        <InlineOverrideField
          value={tool.name}
          originalValue={originalToolName}
          overridden={override.nameOverridden}
          label={t("overrideDisplayName")}
          disabled={!override.enabled}
          busy={override.busy}
          className="w-full"
          textClassName="font-mono text-xl font-semibold tracking-tight"
          onSave={override.saveDisplayName}
        />
      </TypographyH2>
      {override.enabled ? (
        <InlineOverrideField
          value={tool.description || ""}
          originalValue={
            override.descriptionOverridden ? undefined : tool.description || ""
          }
          overridden={override.descriptionOverridden}
          label={t("overrideDescription")}
          multiline
          busy={override.busy}
          className="w-full"
          textClassName="break-words text-sm leading-relaxed"
          onSave={override.saveDescription}
        >
          {description}
        </InlineOverrideField>
      ) : (
        <ToolDescriptionBlock
          description={tool.description}
          showLabel={false}
          omitStructuredSections
          className="text-foreground"
        />
      )}
    </header>
  )
}

export function ToolDetailDocBody({
  tool,
  toolArgs,
  onToolArgChange,
  className,
  serviceName,
  scope,
  originalToolName,
}: {
  tool: ToolInfo
  toolArgs: Record<string, unknown>
  onToolArgChange: (name: string, value: unknown) => void
  className?: string
  serviceName?: string
  scope?: ScopeRef
  originalToolName?: string
}) {
  const { t } = useI18n()
  const { proseInputParams, proseOutputParams, proseReturnSummary } = extractToolDescriptionDocs(
    tool.description || "",
  )
  const inputSchema = getToolSchema(tool) as JsonSchema
  const outputSchema = getToolOutputSchema(tool) as JsonSchema
  const inputParams = schemaFields(inputSchema)
  const outputParams = schemaFields(outputSchema)
  const hasOutputSchema =
    outputParams.length > 0 || Object.keys(outputSchema.properties || {}).length > 0

  return (
    <div className={cn("min-w-0 space-y-6 pb-1 pt-4", className)}>
      <section>
        <TypographyH2>{t("queryParameters")}</TypographyH2>
        <ToolParameterDocList
          fields={inputParams}
          required={inputSchema.required}
          emptyMessage={t("toolNoArgsRunDirectly")}
          values={toolArgs}
          onChange={onToolArgChange}
          proseParamDocs={proseInputParams}
          serviceName={serviceName}
          scope={scope}
          originalToolName={originalToolName || getOriginalToolName(tool)}
        />
      </section>

      <section>
        <TypographyH2>{t("responses")}</TypographyH2>
        <Accordion type="single" collapsible defaultValue="200">
          <AccordionItem value="200" className="border-b-0">
            <AccordionTrigger className="py-3 font-mono text-sm hover:no-underline">
              <span className="font-semibold text-foreground">200</span>
              <span className="ml-2 font-normal text-muted-foreground">{t("responseOk")}</span>
            </AccordionTrigger>
            <AccordionContent>
              {hasOutputSchema ? (
                <div className="flex flex-col gap-3">
                  {proseReturnSummary ? (
                    <p className="text-sm leading-relaxed text-muted-foreground">
                      {proseReturnSummary}
                    </p>
                  ) : null}
                  <ToolParameterDocList
                    fields={outputParams}
                    required={outputSchema.required}
                    emptyMessage={t("noOutputSchema")}
                    proseParamDocs={proseOutputParams}
                  />
                </div>
              ) : proseReturnSummary ? (
                <p className="text-sm leading-relaxed text-muted-foreground">
                  {proseReturnSummary}
                </p>
              ) : (
                <p className="text-sm text-muted-foreground">{t("noOutputSchema")}</p>
              )}
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      </section>

      <ToolAnnotationsSection tool={tool} />
      <ToolMetaSection tool={tool} />
      {serviceName && scope ? (
        <OverrideEditor
          kind="tool"
          serviceName={serviceName}
          componentKey={originalToolName || getOriginalToolName(tool)}
          scope={scope}
        />
      ) : null}
    </div>
  )
}

export function ToolPlaygroundAside({
  tool,
  instanceId,
  toolArgs,
  toolArgsSchema,
  className,
}: {
  tool: ToolInfo
  instanceId: string
  toolArgs: Record<string, unknown>
  toolArgsSchema: ToolSchema
  className?: string
}) {
  const { cliCommand, requestArgsText } = useToolPlaygroundData({
    tool,
    instanceId,
    toolArgs,
    toolArgsSchema,
  })

  return (
    <aside
      className={cn(
        toolDetailPlaygroundAside,
        "h-full min-h-0 min-w-0 w-full flex-col gap-4",
        className,
      )}
    >
      <ToolRequestPanel command={cliCommand} toolName={tool.name} />
      <ToolResponsePanel requestArgsText={requestArgsText} />
    </aside>
  )
}

function ToolRequestPanel({ command, toolName }: { command: string; toolName: string }) {
  const { t } = useI18n()

  return (
    <CodeBlock variant="request" className="flex shrink-0 flex-col">
      <CodeBlockHeader variant="request">
        <CodeBlockTitle>
          <CodeBlockMethod className="bg-sky-500/15 text-sky-300">CALL</CodeBlockMethod>
          <span className="truncate text-zinc-300">{toolName}</span>
        </CodeBlockTitle>
        <CodeBlockActions>
          <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">
            {t("cliRequestLabel")}
          </span>
          <CodeBlockCopyButton
            value={command}
            className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50"
          />
        </CodeBlockActions>
      </CodeBlockHeader>
      <CodeBlockBody variant="request" maxHeight="11rem">
        <CodeBlockString lines={command.split("\n")} />
      </CodeBlockBody>
    </CodeBlock>
  )
}

function ToolResponsePanel({ requestArgsText }: { requestArgsText: string }) {
  const { t } = useI18n()

  return (
    <CodeBlock variant="response" className="flex min-h-0 flex-1 flex-col">
      <CodeBlockHeader variant="response" className="shrink-0">
        <CodeBlockTitle>
          <span className="font-semibold text-foreground">{t("dynamicValue")}</span>
        </CodeBlockTitle>
        <CodeBlockActions>
          <CodeBlockCopyButton value={requestArgsText} />
        </CodeBlockActions>
      </CodeBlockHeader>
      <CodeBlockBody variant="response" fill nativeScroll>
        <CodeBlockJson text={requestArgsText} />
      </CodeBlockBody>
      <CodeBlockFooter variant="response" className="shrink-0">
        <TypographyLead className="m-0 text-xs text-muted-foreground">
          {t("requestPreview")}
        </TypographyLead>
      </CodeBlockFooter>
    </CodeBlock>
  )
}
