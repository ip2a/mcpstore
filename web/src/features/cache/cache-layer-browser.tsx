import { useEffect, useState } from "react"
import type { LucideIcon } from "lucide-react"
import {
  ActivityIcon,
  BotIcon,
  BoxIcon,
  ClipboardIcon,
  ClockIcon,
  DatabaseIcon,
  FolderIcon,
  HardDriveIcon,
  HistoryIcon,
  KeyIcon,
  LinkIcon,
  MonitorIcon,
  UploadIcon,
  WrenchIcon,
} from "lucide-react"
import { toast } from "sonner"

import type { CacheCollectionNode, CacheKeyEntry } from "@/features/cache/cache-model"
import { EntityRow } from "@/components/shared/entity-row"
import { JsonBlock } from "@/components/shared/json-block"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { SelectableRowButton } from "@/components/shared/selectable-row-button"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { useI18n } from "@/lib/i18n-context"

const COLLECTION_TYPE_ICONS: Record<string, LucideIcon> = {
  agents: BotIcon,
  clients: MonitorIcon,
  services: BoxIcon,
  sessions: ClockIcon,
  store: HardDriveIcon,
  tools: WrenchIcon,
  agent_services: LinkIcon,
  service_tools: WrenchIcon,
  session_services: LinkIcon,
  session_tools: WrenchIcon,
  service_status: ActivityIcon,
  service_metadata: DatabaseIcon,
  session_status: ActivityIcon,
  session_state: ClockIcon,
  session_context: ClockIcon,
  context_tool_visibility: KeyIcon,
  tool_preferences: WrenchIcon,
  tool_transforms: WrenchIcon,
  openapi_import_context: UploadIcon,
  openapi_imports: UploadIcon,
  session_events: HistoryIcon,
}

export function collectionTypeIcon(type: string): LucideIcon {
  return COLLECTION_TYPE_ICONS[type] ?? FolderIcon
}

export function firstCollection(collections: CacheCollectionNode[]): string {
  return collections[0]?.collection ?? ""
}

function CacheKeyDetailDialog({
  entry,
  onOpenChange,
}: {
  entry: CacheKeyEntry | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()

  async function onCopy() {
    if (!entry) return
    await navigator.clipboard.writeText(JSON.stringify(entry.value, null, 2))
    toast.success(t("copy"))
  }

  return (
    <Dialog open={Boolean(entry)} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85vh,720px)] flex-col gap-4 overflow-hidden sm:max-w-2xl">
        <DialogHeader className="shrink-0">
          <DialogTitle className="truncate font-mono text-sm" title={entry?.key}>
            {entry?.key}
          </DialogTitle>
          <DialogDescription className="truncate font-mono text-xs" title={entry?.collection}>
            {entry ? `${entry.type} · ${entry.collection}` : null}
          </DialogDescription>
        </DialogHeader>
        <div className="min-h-0 shrink overflow-hidden">
          {entry ? <JsonBlock value={entry.value} className="h-[min(55vh,480px)] max-h-none" /> : null}
        </div>
        <DialogFooter className="shrink-0 sm:justify-between">
          <p className="text-xs text-muted-foreground">{t("cacheValue")}</p>
          <Button size="sm" variant="outline" disabled={!entry} onClick={onCopy}>
            <ClipboardIcon data-icon="inline-start" />
            {t("copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function CacheKeyRow({
  entry,
  onView,
}: {
  entry: CacheKeyEntry
  onView: (entry: CacheKeyEntry) => void
}) {
  const { t } = useI18n()
  const valuePreview = summarizeCacheValue(entry.value)

  return (
    <EntityRow
      variant="inline"
      className="min-h-14 cursor-pointer py-2.5 hover:bg-muted/60"
      tabIndex={0}
      onClick={() => onView(entry)}
      onKeyDown={(event) => {
        if (event.target !== event.currentTarget) return
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault()
          onView(entry)
        }
      }}
      actions={
        <Button variant="outline" size="sm" onClick={() => onView(entry)}>
          {t("view")}
        </Button>
      }
      actionsProps={{ onClick: (event) => event.stopPropagation() }}
    >
      <div className="min-w-0">
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-1">
          <span className="min-w-0 truncate font-mono text-sm font-semibold" title={entry.key}>
            {entry.key}
          </span>
        </div>
        {valuePreview ? (
          <p className="mt-1 truncate font-mono text-xs text-muted-foreground" title={valuePreview}>
            {valuePreview}
          </p>
        ) : null}
      </div>
    </EntityRow>
  )
}

function summarizeCacheValue(value: Record<string, unknown>): string {
  if (!value || Object.keys(value).length === 0) return ""
  try {
    const text = JSON.stringify(value)
    return text.length > 120 ? `${text.slice(0, 117)}...` : text
  } catch {
    return String(value)
  }
}

function CacheCollectionListItem({
  collection,
  selected,
  onSelect,
}: {
  collection: CacheCollectionNode
  selected: boolean
  onSelect: (collection: string) => void
}) {
  return (
    <SelectableRowButton
      meta={collection.collection}
      onClick={() => onSelect(collection.collection)}
      selected={selected}
      title={collection.type}
      trailing={collection.keyCount > 0 ? <Badge variant="outline">{collection.keyCount}</Badge> : null}
    />
  )
}

function CacheCollectionEmptyState({ collection }: { collection: CacheCollectionNode }) {
  const { t } = useI18n()
  const Icon = collectionTypeIcon(collection.type)

  return (
    <Empty className="h-full min-h-[420px] rounded-md border border-dashed">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <Icon />
        </EmptyMedia>
        <EmptyTitle>{collection.type}</EmptyTitle>
        <EmptyDescription>{t("cacheNoKeys")}</EmptyDescription>
      </EmptyHeader>
      <p className="max-w-md truncate font-mono text-xs text-muted-foreground" title={collection.collection}>
        {collection.collection}
      </p>
    </Empty>
  )
}

export function CacheCollectionNav({
  collections,
  value,
  onValueChange,
}: {
  collections: CacheCollectionNode[]
  value: string
  onValueChange: (value: string) => void
}) {
  const { t } = useI18n()

  if (collections.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("cacheEmptyLayer")}</p>
  }

  return (
    <ScrollPane className="min-h-0 flex-1" innerClassName="flex flex-col gap-2 pe-1">
      {collections.map((collection) => (
        <CacheCollectionListItem
          key={collection.collection}
          collection={collection}
          selected={value === collection.collection}
          onSelect={onValueChange}
        />
      ))}
    </ScrollPane>
  )
}

export function CacheCollectionDetail({ collection }: { collection: CacheCollectionNode | null }) {
  const { t } = useI18n()
  const [viewEntry, setViewEntry] = useState<CacheKeyEntry | null>(null)

  useEffect(() => {
    setViewEntry(null)
  }, [collection?.collection])

  if (!collection) {
    return <p className="text-sm text-muted-foreground">{t("cacheSelectCollectionHint")}</p>
  }

  if (collection.keys.length === 0) {
    return <CacheCollectionEmptyState collection={collection} />
  }

  return (
    <>
      <ScrollPane className="min-h-0 flex-1" innerClassName="pe-1">
        <div className="border-t">
          {collection.keys.map((entry) => (
            <CacheKeyRow key={entry.id} entry={entry} onView={setViewEntry} />
          ))}
        </div>
      </ScrollPane>
      <CacheKeyDetailDialog
        entry={viewEntry}
        onOpenChange={(open) => {
          if (!open) setViewEntry(null)
        }}
      />
    </>
  )
}
