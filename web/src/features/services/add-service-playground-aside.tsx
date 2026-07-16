import { useEffect, useMemo, useState } from "react"

import { buildAddServiceCliCommand } from "@/features/services/build-add-service-cli"
import {
  configToFields,
  parseImportedServiceConfig,
  serializeConfigFields,
  type ServiceConfigFields,
  type ServiceConfigFormat,
} from "@/features/services/service-config-draft"
import type { AddServiceScope } from "@/features/services/use-add-service-form"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockBody,
  CodeBlockCopyButton,
  CodeBlockFooter,
  CodeBlockHeader,
  CodeBlockMethod,
  CodeBlockString,
  CodeBlockTitle,
} from "@/components/ui/code-block"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { TypographyLead } from "@/components/ui/typography"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

export function AddServicePlaygroundAside({
  agentId,
  className,
  fields,
  name,
  onFieldsChange,
  onNameChange,
  previewFormat,
  onPreviewFormatChange,
  scope,
}: {
  agentId: string
  className?: string
  fields: ServiceConfigFields
  name: string
  onFieldsChange: (fields: ServiceConfigFields) => void
  onNameChange: (name: string) => void
  previewFormat: ServiceConfigFormat
  onPreviewFormatChange: (format: ServiceConfigFormat) => void
  scope: AddServiceScope
}) {
  const { t } = useI18n()
  const cliCommand = useMemo(
    () => buildAddServiceCliCommand({ name, scope, agentId, fields }),
    [agentId, fields, name, scope],
  )
  const serialized = useMemo(() => serializeConfigFields(fields, previewFormat), [fields, previewFormat])
  const [previewText, setPreviewText] = useState(serialized)
  const [parseError, setParseError] = useState<string | null>(null)
  const [previewDirty, setPreviewDirty] = useState(false)

  useEffect(() => {
    if (!previewDirty) {
      setPreviewText(serialized)
      setParseError(null)
    }
  }, [previewDirty, serialized])

  useEffect(() => {
    setPreviewDirty(false)
  }, [previewFormat])

  function onPreviewChange(text: string) {
    setPreviewText(text)
    setPreviewDirty(true)
    try {
      const imported = parseImportedServiceConfig(previewFormat, text)
      onFieldsChange(configToFields(imported.config))
      if (imported.name?.trim()) {
        onNameChange(imported.name.trim())
      }
      setParseError(null)
      setPreviewDirty(false)
    } catch (err) {
      setParseError(err instanceof Error ? err.message : "Invalid config")
    }
  }

  const serviceName = name.trim() || "github"
  const previewHint =
    parseError || (previewFormat === "json" ? t("fieldHelpJson") : t("fieldHelpToml"))

  return (
    <aside className={cn("flex h-full min-h-0 min-w-0 w-full flex-col gap-3 @min-[640px]:gap-4", className)}>
      <CodeBlock variant="request" className="flex shrink-0 flex-col">
        <CodeBlockHeader variant="request">
          <CodeBlockTitle>
            <CodeBlockMethod className="bg-sky-500/15 text-sky-300">ADD</CodeBlockMethod>
            <span className="truncate text-zinc-300">{serviceName}</span>
          </CodeBlockTitle>
          <CodeBlockActions>
            <span className="rounded-md border border-zinc-700 px-2 py-1 font-mono text-[11px] text-zinc-400">
              {t("cliRequestLabel")}
            </span>
            <CodeBlockCopyButton value={cliCommand} className="text-zinc-300 hover:bg-zinc-800 hover:text-zinc-50" />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="request" maxHeight="9rem">
          <CodeBlockString lines={cliCommand.split("\n")} />
        </CodeBlockBody>
      </CodeBlock>

      <CodeBlock variant="response" className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <CodeBlockHeader variant="response" className="shrink-0">
          <Tabs
            value={previewFormat}
            onValueChange={(value) => onPreviewFormatChange(value as ServiceConfigFormat)}
            className="gap-0"
          >
            <TabsList className="h-8 w-fit">
              <TabsTrigger value="json">json</TabsTrigger>
              <TabsTrigger value="toml">toml</TabsTrigger>
            </TabsList>
          </Tabs>
          <CodeBlockActions>
            <CodeBlockCopyButton value={previewText} />
          </CodeBlockActions>
        </CodeBlockHeader>
        <CodeBlockBody variant="response" fill bare className="p-0">
          <textarea
            value={previewText}
            spellCheck={false}
            className="block h-full min-h-0 w-full flex-1 resize-none overflow-auto border-0 bg-transparent p-4 font-mono text-sm leading-relaxed text-foreground outline-none"
            onChange={(event) => onPreviewChange(event.target.value)}
          />
        </CodeBlockBody>
        <CodeBlockFooter variant="response" className="shrink-0">
          <TypographyLead className={cn("m-0 text-xs", parseError ? "text-destructive" : "text-muted-foreground")}>
            {previewHint}
          </TypographyLead>
        </CodeBlockFooter>
      </CodeBlock>
    </aside>
  )
}