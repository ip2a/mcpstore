import { useState, useEffect } from "react"
import { toast } from "sonner"
import { CheckIcon, XIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { ScrollArea } from "@/components/ui/scroll-area"
import {
  listInstanceTools,
  setInstanceToolPolicy,
  clearInstanceToolPolicy,
  type ServiceInstance,
} from "@/lib/api"
import { useI18n } from "@/lib/i18n-context"

export function ToolPolicyDialog({
  open,
  onOpenChange,
  service,
  allTools,
  onSuccess,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  service: ServiceInstance
  allTools: Array<{ name: string; title?: string | null; description: string }>
  onSuccess?: () => void
}) {
  const { t } = useI18n()
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [availableTools, setAvailableTools] = useState<Set<string>>(new Set())

  useEffect(() => {
    if (!open || !service) return

    async function load() {
      setLoading(true)
      try {
        const tools = await listInstanceTools(service, "available")
        setAvailableTools(new Set(tools.map((tool) => tool.name)))
      } catch (err) {
        toast.error(err instanceof Error ? err.message : "Failed to load tool policy")
      } finally {
        setLoading(false)
      }
    }

    void load()
  }, [open, service])

  function toggleTool(toolName: string) {
    setAvailableTools((prev) => {
      const next = new Set(prev)
      if (next.has(toolName)) {
        next.delete(toolName)
      } else {
        next.add(toolName)
      }
      return next
    })
  }

  async function handleSave() {
    setSaving(true)
    try {
      await setInstanceToolPolicy(service, Array.from(availableTools))
      toast.success(t("toolPolicySaved"))
      onSuccess?.()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to save tool policy")
    } finally {
      setSaving(false)
    }
  }

  async function handleReset() {
    setSaving(true)
    try {
      await clearInstanceToolPolicy(service)
      toast.success(t("toolPolicyReset"))
      onSuccess?.()
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to reset tool policy")
    } finally {
      setSaving(false)
    }
  }

  const availableCount = availableTools.size
  const totalCount = allTools.length

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("toolPolicyTitle")}</DialogTitle>
          <DialogDescription>
            {t("toolPolicyDescription", { service: service.service_name })}
          </DialogDescription>
        </DialogHeader>

        <div className="py-2">
          <div className="mb-3 flex items-center justify-between text-sm">
            <span className="text-muted-foreground">
              {t("toolPolicyAvailableCount", { available: availableCount, total: totalCount })}
            </span>
            <div className="flex gap-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAvailableTools(new Set(allTools.map((tool) => tool.name)))}
                disabled={loading || saving}
              >
                <CheckIcon className="size-3.5" />
                {t("selectAll")}
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setAvailableTools(new Set())}
                disabled={loading || saving}
              >
                <XIcon className="size-3.5" />
                {t("selectNone")}
              </Button>
            </div>
          </div>

          <ScrollArea className="h-[400px] rounded-md border">
            <div className="p-4 space-y-1">
              {loading ? (
                <div className="text-sm text-muted-foreground">{t("loading")}</div>
              ) : (
                allTools.map((tool) => {
                  const displayName = tool.title?.trim() || tool.name
                  const isAvailable = availableTools.has(tool.name)

                  return (
                    <div
                      key={tool.name}
                      className="flex items-center gap-3 hover:bg-muted/50 rounded-md p-2"
                    >
                      <div className="flex-1 min-w-0">
                        <div className="font-mono text-sm font-medium">{displayName}</div>
                        {tool.description ? (
                          <div className="mt-0.5 text-xs text-muted-foreground line-clamp-2">
                            {tool.description}
                          </div>
                        ) : null}
                      </div>
                      <Switch
                        checked={isAvailable}
                        onCheckedChange={() => toggleTool(tool.name)}
                        disabled={saving}
                      />
                    </div>
                  )
                })
              )}
            </div>
          </ScrollArea>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleReset} disabled={loading || saving}>
            {t("reset")}
          </Button>
          <Button onClick={handleSave} disabled={loading || saving}>
            {saving ? t("saving") : t("save")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
