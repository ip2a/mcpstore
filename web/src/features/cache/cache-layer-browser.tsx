import { useEffect, useMemo, useState } from "react"
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
  SearchIcon,
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
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"
import { Input } from "@/components/ui/input"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { CacheValueTree } from "@/features/cache/cache-value-tree"
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
  tool_overrides: WrenchIcon,
  openapi_import_context: UploadIcon,
  openapi_imports: UploadIcon,
  session_events: HistoryIcon,
}

export function collectionTypeIcon(type: string): LucideIcon {
  return COLLECTION_TYPE_ICONS[type] ?? FolderIcon
}

function parseCollectionDisplay(collection: string): {
  namespace: string
  layer: string
  type: string
} {
  const parts = collection.split(":")
  if (parts.length >= 3) {
    return {
      namespace: parts[0],
      layer: parts[1],
      type: parts.slice(2).join(":"),
    }
  }
  return { namespace: "", layer: "", type: collection }
}

export function firstCollection(collections: CacheCollectionNode[]): string {
  return collections[0]?.collection ?? ""
}

function extractKeyFields(value: Record<string, unknown>): { primary: string; secondary?: string } {
  // Try common identification fields
  const name = value.name ?? value.id ?? value.key ?? value.title
  const description = value.description ?? value.type ?? value.status

  if (typeof name === "string") {
    return {
      primary: name,
      secondary: typeof description === "string" ? description : undefined,
    }
  }

  // Fallback to first string field
  const firstString = Object.entries(value).find(([, v]) => typeof v === "string")
  return {
    primary: firstString ? String(firstString[1]) : "{...}",
  }
}

function CacheKeyDetailDialog({
  entry,
  onOpenChange,
}: {
  entry: CacheKeyEntry | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const [viewMode, setViewMode] = useState<"tree" | "json">("tree")

  async function onCopy() {
    if (!entry) return
    await navigator.clipboard.writeText(JSON.stringify(entry.value, null, 2))
    toast.success(t("copy"))
  }

  const parsed = entry ? parseCollectionDisplay(entry.collection) : null

  return (
    <Dialog open={Boolean(entry)} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85vh,800px)] flex-col gap-4 overflow-hidden sm:max-w-3xl">
        <DialogHeader className="shrink-0 space-y-3">
          <div className="flex min-w-0 items-start gap-3">
            <DialogTitle
              className="min-w-0 flex-1 break-all font-mono text-base"
              title={entry?.key}
            >
              {entry?.key}
            </DialogTitle>
            <Badge variant="secondary" className="shrink-0">
              {parsed?.type}
            </Badge>
          </div>
          {parsed ? (
            <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
              <span className="font-mono">{parsed.layer}</span>
              <span>·</span>
              <span className="font-mono">{parsed.namespace}</span>
            </div>
          ) : null}
        </DialogHeader>

        <Tabs
          value={viewMode}
          onValueChange={(v) => setViewMode(v as "tree" | "json")}
          className="flex min-h-0 flex-1 flex-col"
        >
          <TabsList variant="line" className="h-9 shrink-0">
            <TabsTrigger value="tree">Tree View</TabsTrigger>
            <TabsTrigger value="json">Raw JSON</TabsTrigger>
          </TabsList>

          <TabsContent value="tree" className="mt-3 min-h-0 flex-1">
            <ScrollPane className="h-[min(50vh,500px)] rounded-md border bg-muted/30 p-3">
              {entry ? <CacheValueTree value={entry.value} /> : null}
            </ScrollPane>
          </TabsContent>

          <TabsContent value="json" className="mt-3 min-h-0 flex-1">
            {entry ? (
              <JsonBlock value={entry.value} className="h-[min(50vh,500px)] max-h-none" />
            ) : null}
          </TabsContent>
        </Tabs>

        <DialogFooter className="shrink-0 sm:justify-between">
          <p className="text-xs text-muted-foreground">
            {Object.keys(entry?.value ?? {}).length} fields
          </p>
          <Button size="sm" variant="outline" disabled={!entry} onClick={onCopy}>
            <ClipboardIcon data-icon="inline-start" />
            {t("copy")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function fieldToText(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean") return String(value)
  return null
}

/** Pulls a short list of the most useful scalar fields to preview inline, skipping noisy keys. */
function previewFields(value: Record<string, unknown>): Array<[string, string]> {
  const skip = new Set(["name", "id"])
  const preferredOrder = [
    "status",
    "type",
    "state",
    "url",
    "transport",
    "enabled",
    "count",
    "updated_at",
    "created_at",
  ]
  const entries: Array<[string, string]> = []

  for (const field of preferredOrder) {
    if (field in value && !skip.has(field)) {
      const text = fieldToText(value[field])
      if (text !== null) entries.push([field, text])
    }
    if (entries.length >= 3) break
  }

  if (entries.length < 3) {
    for (const [key, raw] of Object.entries(value)) {
      if (entries.length >= 3) break
      if (skip.has(key) || entries.some(([k]) => k === key)) continue
      const text = fieldToText(raw)
      if (text !== null) entries.push([key, text])
    }
  }

  return entries
}

function CacheKeyRow({
  entry,
  onView,
}: {
  entry: CacheKeyEntry
  onView: (entry: CacheKeyEntry) => void
}) {
  const { t } = useI18n()
  const title = fieldToText(entry.value.name) ?? entry.key
  const showKeyAsSubtitle = title !== entry.key
  const fields = previewFields(entry.value)
  const fieldCount = Object.keys(entry.value).length

  return (
    <EntityRow
      variant="inline"
      interactive
      className="cursor-pointer"
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
        <div className="flex min-w-0 flex-wrap items-baseline gap-x-2 gap-y-0.5">
          <span className="min-w-0 truncate text-sm font-semibold" title={title}>
            {title}
          </span>
          <Badge variant="outline" className="shrink-0 font-mono text-[10px]">
            {fieldCount} {t("cacheFields")}
          </Badge>
        </div>
        {showKeyAsSubtitle ? (
          <p
            className="mt-0.5 truncate font-mono text-[11px] text-muted-foreground/80"
            title={entry.key}
          >
            {entry.key}
          </p>
        ) : null}
        {fields.length > 0 ? (
          <div className="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
            {fields.map(([key, value]) => (
              <span key={key} className="inline-flex min-w-0 items-center gap-1 text-xs">
                <span className="text-muted-foreground">{key}</span>
                <span className="min-w-0 truncate font-mono text-foreground/90" title={value}>
                  {value}
                </span>
              </span>
            ))}
          </div>
        ) : null}
      </div>
    </EntityRow>
  )
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
  const Icon = collectionTypeIcon(collection.type)
  const label = collection.type.replaceAll("_", " ")

  return (
    <SelectableRowButton
      leading={<Icon className="size-4 text-muted-foreground" />}
      onClick={() => onSelect(collection.collection)}
      selected={selected}
      title={<span className="capitalize">{label}</span>}
      trailing={
        <Badge variant={collection.keyCount > 0 ? "outline" : "secondary"}>
          {collection.keyCount}
        </Badge>
      }
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
      <p
        className="max-w-md truncate font-mono text-xs text-muted-foreground"
        title={collection.collection}
      >
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
  const [search, setSearch] = useState("")

  const filtered = useMemo(() => {
    if (!search.trim()) return collections
    const query = search.toLowerCase()
    return collections.filter(
      (c) => c.type.toLowerCase().includes(query) || c.collection.toLowerCase().includes(query),
    )
  }, [collections, search])

  if (collections.length === 0) {
    return <p className="text-sm text-muted-foreground">{t("cacheEmptyLayer")}</p>
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2">
      {collections.length > 5 ? (
        <div className="relative shrink-0">
          <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            type="search"
            placeholder={t("search")}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-9 pl-8"
          />
        </div>
      ) : null}
      <ScrollPane className="min-h-0 flex-1" innerClassName="flex flex-col gap-2 pe-1">
        {filtered.length === 0 ? (
          <p className="py-8 text-center text-sm text-muted-foreground">{t("noResults")}</p>
        ) : (
          filtered.map((collection) => (
            <CacheCollectionListItem
              key={collection.collection}
              collection={collection}
              selected={value === collection.collection}
              onSelect={onValueChange}
            />
          ))
        )}
      </ScrollPane>
    </div>
  )
}

export function CacheCollectionDetail({ collection }: { collection: CacheCollectionNode | null }) {
  const { t } = useI18n()
  const [viewEntry, setViewEntry] = useState<CacheKeyEntry | null>(null)
  const [keySearch, setKeySearch] = useState("")

  const filteredKeys = useMemo(() => {
    if (!collection) return []
    if (!keySearch.trim()) return collection.keys
    const query = keySearch.toLowerCase()
    return collection.keys.filter((entry) => {
      const title = fieldToText(entry.value.name) ?? entry.key
      return title.toLowerCase().includes(query) || entry.key.toLowerCase().includes(query)
    })
  }, [collection, keySearch])

  useEffect(() => {
    setViewEntry(null)
    setKeySearch("")
  }, [collection?.collection])

  if (!collection) {
    return (
      <div className="flex h-full min-h-[320px] items-center justify-center rounded-md border border-dashed">
        <p className="text-sm text-muted-foreground">{t("cacheSelectCollectionHint")}</p>
      </div>
    )
  }

  if (collection.keys.length === 0) {
    return <CacheCollectionEmptyState collection={collection} />
  }

  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col gap-3">
        {collection.keys.length > 4 ? (
          <div className="relative shrink-0">
            <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              type="search"
              placeholder={t("searchKeys")}
              value={keySearch}
              onChange={(e) => setKeySearch(e.target.value)}
              className="h-9 pl-8"
            />
          </div>
        ) : null}
        <ScrollPane className="min-h-0 flex-1" innerClassName="pe-1">
          {filteredKeys.length === 0 ? (
            <div className="flex min-h-[240px] items-center justify-center rounded-md border border-dashed">
              <p className="text-sm text-muted-foreground">{t("noResults")}</p>
            </div>
          ) : (
            <div className="border-t">
              {filteredKeys.map((entry) => (
                <CacheKeyRow key={entry.id} entry={entry} onView={setViewEntry} />
              ))}
            </div>
          )}
        </ScrollPane>
      </div>
      <CacheKeyDetailDialog
        entry={viewEntry}
        onOpenChange={(open) => {
          if (!open) setViewEntry(null)
        }}
      />
    </>
  )
}
