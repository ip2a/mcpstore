import { useState } from "react"
import { PencilIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { OverrideEditor } from "@/features/overrides/override-editor"
import type { ServiceInstance, ScopeView } from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

export function ToolConfigRow({
  service,
  tool,
  scope,
}: {
  service: ServiceInstance
  tool: { name: string; description: string; title?: string | null }
  scope: ScopeView
}) {
  const { t } = useI18n()
  const [editing, setEditing] = useState(false)
  const displayName = tool.title?.trim() || tool.name

  const scopeRef =
    scope.type === "root" || scope.type === "store"
      ? { type: "store" as const }
      : { type: "agent" as const, agent_id: scope.agent_id }

  return (
    <>
      <div className="flex items-center gap-3 border-b border-border/50 px-4 py-2 last:border-0 hover:bg-muted/30">
        <span className="min-w-0 flex-1">
          <span className="block font-mono text-sm font-medium">{displayName}</span>
          {tool.description ? (
            <span className="mt-0.5 block text-xs text-muted-foreground line-clamp-1">
              {tool.description}
            </span>
          ) : null}
        </span>

        <Button
          size="sm"
          variant="ghost"
          className="shrink-0"
          onClick={() => setEditing(true)}
          title={t("edit")}
        >
          <PencilIcon className="size-3.5" />
        </Button>
      </div>

      <Dialog open={editing} onOpenChange={setEditing}>
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {t("editTool")}: {displayName}
            </DialogTitle>
          </DialogHeader>
          <OverrideEditor
            kind="tool"
            serviceName={service.service_name}
            componentKey={tool.name}
            scope={scopeRef}
            onSuccess={() => setEditing(false)}
          />
        </DialogContent>
      </Dialog>
    </>
  )
}
