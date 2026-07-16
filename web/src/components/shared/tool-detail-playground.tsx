import { useMemo, useState } from "react"
import { PlayIcon } from "lucide-react"

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
import { ToolAnnotationsSection, ToolMetaSection } from "@/components/shared/tool-capability-sections"
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Switch } from "@/components/ui/switch"
import { TypographyH2, TypographyLead } from "@/components/ui/typography"
import { useI18n } from "@/lib/i18n-context"
import type { ToolInfo } from "@/lib/api"
import { buildSchemaExampleValue, buildToolCliCommand } from "@/lib/tool-schema-preview"
import { serializeToolArgs, type ToolSchema } from "@/lib/tool-args"
import { getToolOutputSchema, getToolSchema, extractToolDescriptionDocs } from "@/lib/tool-info"
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
  const hasOutputSchema = outputParams.length > 0 || Object.keys(outputSchema.properties || {}).length > 0

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

  const exampleResponse = useMemo(() => buildSchemaExampleValue(outputSchema), [outputSchema])
  const responseText = useMemo(
    () =>
      hasOutputSchema
        ? JSON.stringify(exampleResponse, null, 2)
        : JSON.stringify({ content: [{ type: "text", text: "..." }] }, null, 2),
    [exampleResponse, hasOutputSchema],
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
    responseText,
  }
}

export function ToolDetailDocHeader({ tool, className }: { tool: ToolInfo; className?: string }) {
  return (
    <header className={cn("shrink-0 space-y-3 border-b pb-4", className)}>
      <TypographyH2 className="border-b-0 pb-0 break-words font-mono text-xl">{tool.name}</TypographyH2>
      <ToolDescriptionBlock
        description={tool.description}
        showLabel={false}
        omitStructuredSections
        className="text-foreground"
      />
    </header>
  )
}

export function ToolDetailDocBody({
  tool,
  toolArgs,
  onToolArgChange,
  className,
}: {
  tool: ToolInfo
  toolArgs: Record<string, unknown>
  onToolArgChange: (name: string, value: unknown) => void
  className?: string
}) {
  const { t } = useI18n()
  const { proseInputParams, proseOutputParams, proseReturnSummary } = extractToolDescriptionDocs(tool.description || "")
  const inputSchema = getToolSchema(tool) as JsonSchema
  const outputSchema = getToolOutputSchema(tool) as JsonSchema
  const inputParams = schemaFields(inputSchema)
  const outputParams = schemaFields(outputSchema)
  const hasOutputSchema = outputParams.length > 0 || Object.keys(outputSchema.properties || {}).length > 0

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
                    <p className="text-sm leading-relaxed text-muted-foreground">{proseReturnSummary}</p>
                  ) : null}
                  <ToolParameterDocList
                    fields={outputParams}
                    required={outputSchema.required}
                    emptyMessage={t("noOutputSchema")}
                    proseParamDocs={proseOutputParams}
                  />
                </div>
              ) : proseReturnSummary ? (
                <p className="text-sm leading-relaxed text-muted-foreground">{proseReturnSummary}</p>
              ) : (
                <p className="text-sm text-muted-foreground">{t("noOutputSchema")}</p>
              )}
            </AccordionContent>
          </AccordionItem>
        </Accordion>
      </section>

      <ToolAnnotationsSection tool={tool} />
      <ToolMetaSection tool={tool} />
    </div>
  )
}

export function ToolPlaygroundAside({
  tool,
  instanceId,
  toolArgs,
  toolArgsSchema,
  onRun,
  running,
  className,
}: {
  tool: ToolInfo
  instanceId: string
  toolArgs: Record<string, unknown>
  toolArgsSchema: ToolSchema
  onRun?: () => void
  running?: boolean
  className?: string
}) {
  const { cliCommand, hasOutputSchema, outputSchema, requestArgsText, responseText } = useToolPlaygroundData({
    tool,
    instanceId,
    toolArgs,
    toolArgsSchema,
  })

  return (
    <aside className={cn("flex h-full min-h-0 min-w-0 w-full flex-col gap-4", className)}>
      <ToolRequestPanel command={cliCommand} toolName={tool.name} onRun={onRun} running={running} />
      <ToolResponsePanel
        requestArgsText={requestArgsText}
        responseText={responseText}
        schema={outputSchema}
        hasOutputSchema={hasOutputSchema}
      />
    </aside>
  )
}

function ToolRequestPanel({
  command,
  toolName,
  onRun,
  running,
}: {
  command: string
  toolName: string
  onRun?: () => void
  running?: boolean
}) {
  const { t } = useI18n()

  return (
    <CodeBlock variant="request" className="flex shrink-0 flex-col">
      <CodeBlockHeader variant="request">
        <CodeBlockTitle>
          <CodeBlockMethod className="bg-sky-500/15 text-sky-300">CALL</CodeBlockMethod>
          <span className="truncate text-zinc-300">{toolName}</span>
        </CodeBlockTitle>
        <CodeBlockActions>
          <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">{t("cliRequestLabel")}</span>
          <CodeBlockCopyButton value={command} className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50" />
        </CodeBlockActions>
      </CodeBlockHeader>
      <CodeBlockBody variant="request" maxHeight="11rem">
        <CodeBlockString lines={command.split("\n")} />
      </CodeBlockBody>
      <CodeBlockFooter variant="request" className="justify-end">
        {onRun ? (
          <Button
            size="sm"
            className="h-8 bg-zinc-100 text-zinc-900 hover:bg-white"
            onClick={onRun}
            disabled={running}
          >
            {running ? <Spinner data-icon="inline-start" /> : <PlayIcon data-icon="inline-start" />}
            {running ? t("executing") : t("testRequest")}
          </Button>
        ) : null}
      </CodeBlockFooter>
    </CodeBlock>
  )
}

function ToolResponsePanel({
  requestArgsText,
  responseText,
  schema,
  hasOutputSchema,
}: {
  requestArgsText: string
  responseText: string
  schema: JsonSchema
  hasOutputSchema: boolean
}) {
  const { t } = useI18n()
  const [showSchema, setShowSchema] = useState(false)
  const displayText = showSchema && hasOutputSchema ? responseText : requestArgsText

  return (
    <CodeBlock variant="response" className="flex min-h-0 flex-1 flex-col">
      <CodeBlockHeader variant="response" className="shrink-0">
        <CodeBlockTitle>
          <span className="font-semibold text-foreground">{showSchema && hasOutputSchema ? "200" : t("dynamicValue")}</span>
        </CodeBlockTitle>
        <CodeBlockActions>
          <div className="flex items-center gap-2">
            <Switch id="show-schema" checked={showSchema} onCheckedChange={setShowSchema} disabled={!hasOutputSchema} />
            <Label htmlFor="show-schema" className="text-xs font-normal text-muted-foreground">
              {t("showSchema")}
            </Label>
          </div>
          <CodeBlockCopyButton value={displayText} />
        </CodeBlockActions>
      </CodeBlockHeader>
      <CodeBlockBody variant="response" fill>
        <CodeBlockJson text={displayText} />
      </CodeBlockBody>
      <CodeBlockFooter variant="response" className="shrink-0">
        <TypographyLead className="m-0 text-xs text-muted-foreground">
          {showSchema && hasOutputSchema ? t("responseOk") : t("requestPreview")}
        </TypographyLead>
      </CodeBlockFooter>
    </CodeBlock>
  )
}
