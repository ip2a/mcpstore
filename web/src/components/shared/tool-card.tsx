import { ClipboardIcon, EyeIcon, WrenchIcon } from "lucide-react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { EntityRow } from "@/components/shared/entity-row"
import { PanelCard } from "@/components/shared/panel-card"
import { SectionHeading } from "@/components/shared/section-heading"
import type { ToolInfo } from "@/lib/api"
import { getToolSchema, getToolServiceName } from "@/lib/tool-info"

export function ToolCard({ tool, sourceLabel, onRun, onDetail }: { tool: ToolInfo; sourceLabel?: string; onRun: () => void; onDetail: () => void }) {
  const schema = getToolSchema(tool) as { properties?: Record<string, { type?: string; description?: string }>; required?: string[] }
  const params = Object.entries(schema.properties || {}).sort(([a], [b]) => a.localeCompare(b))

  async function onCopy() {
    await navigator.clipboard.writeText(JSON.stringify(tool, null, 2))
    toast.success("Tool copied")
  }

  return (
    <PanelCard>
      <SectionHeading
        title={tool.name}
        titleAs="h2"
        description={tool.description || "No description"}
        className="border-b-0 pb-0"
        actions={
          <div className="flex flex-wrap justify-end gap-2">
            <Button size="sm" onClick={onRun}>
              <WrenchIcon data-icon="inline-start" />
              Run
            </Button>
            <Button size="sm" variant="outline" onClick={onDetail}>
              <EyeIcon data-icon="inline-start" />
              Details
            </Button>
            <Button size="sm" variant="outline" onClick={onCopy}>
              <ClipboardIcon data-icon="inline-start" />
              Copy
            </Button>
          </div>
        }
      />
      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap gap-2">
          <Badge variant="secondary">{sourceLabel || getToolServiceName(tool) || "store"}</Badge>
          {schema.required?.length ? <Badge variant="outline">{schema.required.length} required</Badge> : <Badge variant="outline">optional</Badge>}
        </div>
        {params.length ? (
          params.slice(0, 4).map(([name, meta]) => (
            <EntityRow key={name} actions={<Badge variant="outline">{meta.type || "any"}</Badge>}>
              <div className="min-w-0">
                <code className="text-sm font-medium">{name}</code>
                <p className="truncate text-sm text-muted-foreground">{meta.description || "No description"}</p>
              </div>
            </EntityRow>
          ))
        ) : (
          <p className="text-sm text-muted-foreground">No params required</p>
        )}
      </div>
    </PanelCard>
  )
}
