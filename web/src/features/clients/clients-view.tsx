import { useState } from "react"
import { CableIcon, CircleDashed, FileJsonIcon, FolderIcon, TerminalIcon } from "lucide-react"

import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { useClientsQuery, useClientToggle } from "@/features/clients/queries"
import { useI18n } from "@/lib/i18n-context"
import type { ClientInventory, ExposeConfig, WireOptions } from "@/lib/api"
import { cn } from "@/lib/utils"

const EXPOSE_KEYS: (keyof ExposeConfig)[] = [
  "tool",
  "prompt",
  "resource",
  "openapi",
  "service",
  "cache",
  "search",
  "session_state",
]

export function ClientsView() {
  const { t } = useI18n()
  const { data, isPending, isError, error, refetch } = useClientsQuery()
  const { toggle, busy } = useClientToggle()
  const [wireTarget, setWireTarget] = useState<ClientInventory | null>(null)

  if (isError) {
    return (
      <PageError
        title={t("clientsLoadFailed")}
        message={error instanceof Error ? error.message : String(error)}
        onRefresh={() => void refetch()}
      />
    )
  }
  if (isPending) return <PageSkeleton />

  return (
    <ScrollPane className="flex-1" innerClassName="flex flex-col gap-3 pb-2">
      {data.map((client) => (
        <ClientRow
          key={client.id}
          client={client}
          busy={busy}
          onToggle={() => {
            if (client.wired) void toggle(client, true)
            else setWireTarget(client)
          }}
        />
      ))}
      <WireDialog
        client={wireTarget}
        busy={busy}
        onClose={() => setWireTarget(null)}
        onConfirm={(client, options) => {
          setWireTarget(null)
          void toggle(client, false, options)
        }}
      />
    </ScrollPane>
  )
}

/** 接线配置：默认专属作用域（Agent:<id>）+ 不暴露管理工具；可切 Store / 打开开关。 */
function WireDialog({
  client,
  busy,
  onClose,
  onConfirm,
}: {
  client: ClientInventory | null
  busy: boolean
  onClose: () => void
  onConfirm: (client: ClientInventory, options: WireOptions) => void
}) {
  const { t } = useI18n()
  const [dedicatedScope, setDedicatedScope] = useState(true)
  const [expose, setExpose] = useState<ExposeConfig>({
    session_state: false,
    tool: false,
    prompt: false,
    resource: false,
    openapi: false,
    service: false,
    cache: false,
    search: false,
  })
  if (!client) return null
  return (
    <Dialog open onOpenChange={(open) => { if (!open) onClose() }}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("clientsWireDialogTitle", { client: client.label })}</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between gap-3">
            <div className="flex flex-col gap-0.5">
              <Label htmlFor="wire-scope">{t("clientsScope")}</Label>
              <span className="font-mono text-xs text-muted-foreground">
                {dedicatedScope ? `Agent:${client.id}` : t("gatewayScopeStore")}
              </span>
            </div>
            <Switch id="wire-scope" checked={dedicatedScope} onCheckedChange={setDedicatedScope} />
          </div>
          <div className="flex flex-col gap-2">
            <Label>{t("clientsExpose")}</Label>
            <div className="grid grid-cols-2 gap-x-4 gap-y-2">
              {EXPOSE_KEYS.map((key) => (
                <label key={key} className="flex cursor-pointer items-center gap-2 text-sm">
                  <Switch
                    checked={expose[key]}
                    onCheckedChange={(on) => setExpose((prev) => ({ ...prev, [key]: on }))}
                  />
                  <span className="font-mono text-xs">{key}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>{t("cancel")}</Button>
          <Button
            disabled={busy}
            onClick={() =>
              onConfirm(client, {
                scope: dedicatedScope ? "agent" : "store",
                expose,
              })
            }
          >
            {t("clientsWire")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function ClientRow({
  busy,
  client,
  onToggle,
}: {
  busy: boolean
  client: ClientInventory
  onToggle: () => void
}) {
  const { t } = useI18n()

  return (
    <PanelCard variant="plain" className="gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2 border-b pb-3">
        <div className="flex min-w-0 flex-wrap items-center gap-2">
          <CableIcon className="size-4 shrink-0 text-muted-foreground" />
          <strong className="text-sm font-medium">{client.label}</strong>
          <DetectedBadge client={client} />
          {client.wired ? (
            <Badge variant="default">{t("clientsWired")}</Badge>
          ) : null}
        </div>
        {client.wired ? (
          <Button disabled={busy} size="sm" variant="outline" onClick={onToggle}>
            {t("clientsUnwire")}
          </Button>
        ) : (
          <Button disabled={busy} size="sm" onClick={onToggle}>
            {t("clientsWire")}
          </Button>
        )}
      </div>

      <div className="flex flex-col gap-1 font-mono text-xs text-muted-foreground">
        <span className="truncate">
          {t("clientsConfigPath")}: {client.config_path}
          {client.config_exists ? "" : ` (${t("clientsConfigMissing")})`}
        </span>
        {client.wired_url ? (
          <span className="truncate">{t("clientsWiredUrl")}: {client.wired_url}</span>
        ) : null}
        {client.mcp_servers ? (
          <span className="truncate" title={client.mcp_servers.join(", ")}>
            {t("clientsServers")} ({client.mcp_servers.length}):{" "}
            {client.mcp_servers.length > 0 ? client.mcp_servers.join(", ") : "—"}
          </span>
        ) : null}
        {client.unsupported_fields.length > 0 ? (
          <span className="truncate text-amber-600 dark:text-amber-400" title={client.unsupported_fields.join(", ")}>
            {t("clientsUnsupported")}: {client.unsupported_fields.join(", ")}
          </span>
        ) : null}
      </div>
    </PanelCard>
  )
}

function DetectedBadge({ client }: { client: ClientInventory }) {
  const { t } = useI18n()
  const icon =
    client.detected_by === "config_file" ? (
      <FileJsonIcon className="size-3 shrink-0" />
    ) : client.detected_by === "config_dir" ? (
      <FolderIcon className="size-3 shrink-0" />
    ) : client.detected_by === "command" ? (
      <TerminalIcon className="size-3 shrink-0" />
    ) : (
      <CircleDashed className="size-3 shrink-0" />
    )
  return (
    <Badge
      className={cn("gap-1", client.detected_by ? "" : "text-muted-foreground")}
      variant={client.detected_by ? "secondary" : "outline"}
    >
      {icon}
      {client.detected_by ? t("clientsDetected") : t("clientsNotDetected")}
    </Badge>
  )
}
