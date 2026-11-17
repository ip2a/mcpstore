import { useEffect, useMemo, useState } from "react"

import { ConfigPreviewEditor } from "@/features/services/config-preview-editor"
import {
  configToFields,
  parseImportedServiceConfig,
  serializeConfigFields,
  type ServiceConfigFields,
  type ServiceConfigFormat,
} from "@/features/services/service-config-draft"
import {
  CodeBlock,
  CodeBlockActions,
  CodeBlockBody,
  CodeBlockCopyButton,
  CodeBlockFooter,
  CodeBlockHeader,
  CodeBlockString,
} from "@/components/ui/code-block"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { useI18n } from "@/lib/i18n-context"

type PreviewView = "cli" | ServiceConfigFormat

const darkTabTrigger =
  "text-[#d4d4d4] data-[state=active]:bg-white/10 data-[state=active]:text-white"

export function ServiceConfigPreviewCard({
  cliCommand,
  fields,
  onFieldsChange,
  onNameChange,
  previewFormat,
  onPreviewFormatChange,
  showCli = false,
}: {
  cliCommand: string
  fields: ServiceConfigFields
  onFieldsChange: (fields: ServiceConfigFields) => void
  onNameChange?: (name: string) => void
  previewFormat: ServiceConfigFormat
  onPreviewFormatChange: (format: ServiceConfigFormat) => void
  showCli?: boolean
}) {
  const { t } = useI18n()
  const [cliOpen, setCliOpen] = useState(false)
  const serialized = useMemo(
    () => serializeConfigFields(fields, previewFormat),
    [fields, previewFormat],
  )
  const [previewText, setPreviewText] = useState(serialized)
  const [parseError, setParseError] = useState<string | null>(null)
  const [previewDirty, setPreviewDirty] = useState(false)

  useEffect(() => {
    if (!showCli) setCliOpen(false)
  }, [showCli])

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
        onNameChange?.(imported.name.trim())
      }
      setParseError(null)
      setPreviewDirty(false)
    } catch (err) {
      setParseError(err instanceof Error ? err.message : "Invalid config")
    }
  }

  function onTabChange(value: string) {
    if (value === "cli") {
      setCliOpen(true)
      return
    }
    setCliOpen(false)
    onPreviewFormatChange(value as ServiceConfigFormat)
  }

  const activeView: PreviewView = showCli && cliOpen ? "cli" : previewFormat
  const isCli = activeView === "cli"
  const copyValue = isCli ? cliCommand : previewText

  return (
    <CodeBlock
      variant="response"
      className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border-0 bg-[#1d1e22] p-0.5 text-white shadow-[0_4px_30px_rgba(0,0,0,0.5)]"
    >
      <CodeBlockHeader
        variant="response"
        className="shrink-0 border-0 bg-transparent px-2.5 py-2.5 text-[#d4d4d4]"
      >
        <Tabs value={activeView} onValueChange={onTabChange} className="gap-0">
          <TabsList className="h-8 w-fit bg-white/5">
            <TabsTrigger value="json" className={darkTabTrigger}>
              JSON
            </TabsTrigger>
            <TabsTrigger value="toml" className={darkTabTrigger}>
              TOML
            </TabsTrigger>
            {showCli ? (
              <TabsTrigger value="cli" className={darkTabTrigger}>
                {t("cliRequestLabel")}
              </TabsTrigger>
            ) : null}
          </TabsList>
        </Tabs>
        <CodeBlockActions>
          <CodeBlockCopyButton
            value={copyValue}
            className="text-[#d4d4d4] hover:bg-[#6e7281] hover:text-white"
          />
        </CodeBlockActions>
      </CodeBlockHeader>

      <CodeBlockBody variant="response" fill bare className="bg-transparent p-0 text-white">
        {isCli ? (
          <div className="min-h-0 flex-1 overflow-auto px-2.5 pb-2.5 font-mono text-sm leading-relaxed text-[#b6cea8] [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
            <CodeBlockString lines={cliCommand.split("\n")} />
          </div>
        ) : (
          <ConfigPreviewEditor
            value={previewText}
            format={previewFormat}
            onChange={onPreviewChange}
          />
        )}
      </CodeBlockBody>

      {!isCli && parseError ? (
        <CodeBlockFooter
          variant="response"
          className="shrink-0 border-0 border-t border-white/10 bg-transparent px-2.5 py-2 text-red-400"
        >
          <span className="text-xs">{parseError}</span>
        </CodeBlockFooter>
      ) : null}
    </CodeBlock>
  )
}
