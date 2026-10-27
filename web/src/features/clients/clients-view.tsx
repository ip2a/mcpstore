import { useState, type ReactNode } from "react"
import {
  ChevronRightIcon,
  CircleDashed,
  FileJsonIcon,
  FolderIcon,
  TerminalIcon,
} from "lucide-react"

import { clientIconUrls } from "@/assets/icons/clients"
import { PageError, PageSkeleton } from "@/components/shared/page-states"
import { PanelCard } from "@/components/shared/panel-card"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
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
import { useClientsQuery, useClientToggle, useClientVersionQuery } from "@/features/clients/queries"
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

  const clients = data ?? []

  return (
    <>
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("clientsWorkspaceTitle")}</h1>}
        subtitle={t("clientsWorkspaceDescription", { count: clients.length })}
      />

      {isError ? (
        <PageError
          title={t("clientsLoadFailed")}
          message={error instanceof Error ? error.message : String(error)}
          onRefresh={() => void refetch()}
        />
      ) : isPending ? (
        <PageSkeleton />
      ) : (
        <ScrollPane className="flex-1" innerClassName="flex flex-col gap-2 pb-2">
          <PanelCard variant="plain" className="divide-y divide-border/50 p-0">
            {clients.map((client) => (
              <ClientRow
                key={client.id}
                client={client}
                busy={busy}
                onWire={() => setWireTarget(client)}
                onUnwire={() => void toggle(client, true)}
              />
            ))}
          </PanelCard>
        </ScrollPane>
      )}

      <WireDialog
        client={wireTarget}
        busy={busy}
        onClose={() => setWireTarget(null)}
        onConfirm={(client, options) => {
          setWireTarget(null)
          void toggle(client, false, options)
        }}
      />
    </>
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
  onWire,
  onUnwire,
}: {
  busy: boolean
  client: ClientInventory
  onWire: () => void
  onUnwire: () => void
}) {
  const { t } = useI18n()
  const [detailOpen, setDetailOpen] = useState(false)
  const detected = client.detected_by != null
  const mcpCount = client.mcp_servers?.length ?? 0
  const showMcp = client.mcp_servers != null
  const versionQuery = useClientVersionQuery(client.id, detected)
  const version = versionQuery.data ?? client.version ?? null

  const statusText = !detected
    ? t("clientsStatusNotDetected")
    : client.wired
      ? showMcp
        ? t("clientsStatusWired", { client: client.id, count: mcpCount })
        : t("clientsWired")
      : showMcp
        ? t("clientsStatusDetected", { client: client.id, count: mcpCount })
        : t("clientsDetected")

  return (
    <>
      <div className="flex items-center gap-3 px-4 py-3">
        <ClientLogo clientId={client.id} label={client.label} />

        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-baseline gap-2">
            <span className="truncate text-sm font-medium">{client.id}</span>
            {version ? (
              <span className="shrink-0 text-xs tabular-nums text-muted-foreground">{version}</span>
            ) : null}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
            <span
              className={cn(
                "size-1.5 shrink-0 rounded-full",
                client.wired
                  ? "bg-emerald-500"
                  : detected
                    ? "bg-emerald-500/70"
                    : "bg-muted-foreground/40",
              )}
            />
            <span className="truncate">{statusText}</span>
          </div>
        </div>

        {showMcp ? (
          <button
            type="button"
            className="inline-flex shrink-0 items-center gap-0.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label={t("clientsDetailTitle", { client: client.label })}
            onClick={() => setDetailOpen(true)}
          >
            <span>{t("clientsMcpShort")}</span>
            <span className="tabular-nums">{mcpCount}</span>
            <ChevronRightIcon className="size-3.5" />
          </button>
        ) : (
          <button
            type="button"
            className="inline-flex shrink-0 items-center gap-0.5 rounded-md px-2 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label={t("clientsDetailTitle", { client: client.label })}
            onClick={() => setDetailOpen(true)}
          >
            <ChevronRightIcon className="size-3.5" />
          </button>
        )}

        <Switch
          checked={client.wired}
          disabled={busy}
          aria-label={client.wired ? t("clientsUnwire") : t("clientsWire")}
          onCheckedChange={(checked) => {
            if (checked) onWire()
            else onUnwire()
          }}
        />
      </div>

      <ClientDetailDialog client={client} version={version} open={detailOpen} onOpenChange={setDetailOpen} />
    </>
  )
}

function ClientLogo({ clientId, label }: { clientId: string; label: string }) {
  const iconUrl = clientIconUrls[clientId]
  return (
    <span className="flex size-8 shrink-0 items-center justify-center">
      {iconUrl ? (
        <img src={iconUrl} alt="" className="size-7 object-contain" aria-hidden />
      ) : (
        <span
          aria-hidden
          className="flex size-7 items-center justify-center rounded-md bg-muted text-xs font-medium uppercase"
        >
          {clientId.slice(0, 2)}
        </span>
      )}
      <span className="sr-only">{label}</span>
    </span>
  )
}

function ClientDetailDialog({
  client,
  version,
  open,
  onOpenChange,
}: {
  client: ClientInventory
  version: string | null
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("clientsDetailTitle", { client: client.label })}</DialogTitle>
        </DialogHeader>
        <dl className="flex flex-col gap-3 text-sm">
          {version ? (
            <DetailRow label={t("version")}>
              <span className="font-mono text-xs tabular-nums">{version}</span>
            </DetailRow>
          ) : null}
          <DetailRow label={t("clientsConfigPath")}>
            <span className="font-mono text-xs break-all">
              {client.config_path}
              {client.config_exists ? "" : ` (${t("clientsConfigMissing")})`}
            </span>
          </DetailRow>
          {client.wired_url ? (
            <DetailRow label={t("clientsWiredUrl")}>
              <span className="font-mono text-xs break-all">{client.wired_url}</span>
            </DetailRow>
          ) : null}
          <DetailRow label={t("clientsDetected")}>
            <DetectedDetail client={client} />
          </DetailRow>
          {client.mcp_servers != null ? (
            <DetailRow label={t("clientsServers")}>
              {client.mcp_servers.length > 0 ? (
                <ul className="flex flex-col gap-1 font-mono text-xs">
                  {client.mcp_servers.map((name) => (
                    <li key={name}>{name}</li>
                  ))}
                </ul>
              ) : (
                <span className="text-muted-foreground">—</span>
              )}
            </DetailRow>
          ) : null}
          {client.unsupported_fields.length > 0 ? (
            <DetailRow label={t("clientsUnsupported")}>
              <ul className="flex flex-col gap-1 font-mono text-xs text-amber-600 dark:text-amber-400">
                {client.unsupported_fields.map((field) => (
                  <li key={field}>{field}</li>
                ))}
              </ul>
            </DetailRow>
          ) : null}
        </dl>
      </DialogContent>
    </Dialog>
  )
}

function DetailRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1">
      <dt className="text-xs font-medium text-muted-foreground">{label}</dt>
      <dd>{children}</dd>
    </div>
  )
}

function DetectedDetail({ client }: { client: ClientInventory }) {
  const { t } = useI18n()
  const icon =
    client.detected_by === "config_file" ? (
      <FileJsonIcon className="size-3.5 shrink-0" />
    ) : client.detected_by === "config_dir" ? (
      <FolderIcon className="size-3.5 shrink-0" />
    ) : client.detected_by === "command" ? (
      <TerminalIcon className="size-3.5 shrink-0" />
    ) : (
      <CircleDashed className="size-3.5 shrink-0" />
    )

  return (
    <span className="inline-flex items-center gap-1.5 text-muted-foreground">
      {icon}
      {client.detected_by ? t("clientsDetected") : t("clientsNotDetected")}
    </span>
  )
}
