import { useEffect, useMemo, useState } from "react"
import { ChevronLeftIcon, ChevronRightIcon, ClipboardIcon } from "lucide-react"
import { toast } from "sonner"

import {
  LAYER_I18N_KEY,
  type KvCollectionNode,
  type KvKeyEntry,
  type KvLayerNode,
} from "@/features/kv/kv-model"
import { JsonBlock } from "@/components/shared/json-block"
import { ScrollPane } from "@/components/shared/scroll-pane"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { KvValueTree } from "@/features/kv/kv-value-tree"
import {
  TRACE_COL1_CELL_CLASSNAME,
  TRACE_COL1_CLASSNAME,
  TRACE_COL2_CELL_CLASSNAME,
  TRACE_COL2_CLASSNAME,
  TRACE_COL_TAGS_CLASSNAME,
  TRACE_TABLE_CLASSNAME,
} from "@/features/traces/trace-table-layout"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const PAGE_SIZES = [10, 20, 50] as const

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

function fieldToText(value: unknown): string | null {
  if (value === null || value === undefined) return null
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean") return String(value)
  return null
}

function entryName(entry: KvKeyEntry): string {
  return fieldToText(entry.value.name) ?? fieldToText(entry.value.service_name) ?? entry.key
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
}

/** Type as first tag, then a few representative value fields. */
function entryTags(entry: KvKeyEntry, type: string): string[] {
  const value = entry.value
  const tags: string[] = []

  function push(raw: unknown) {
    const text = fieldToText(raw)?.trim()
    if (!text || tags.includes(text)) return
    tags.push(text)
  }

  push(type.replaceAll("_", " "))

  switch (type) {
    case "service_definitions": {
      const base = value.base_config
      push(isRecord(base) ? base.transport : null)
      break
    }
    case "service_instances": {
      push(value.service_name)
      push(value.transport)
      push(isRecord(value.scope) ? value.scope.type : null)
      break
    }
    case "service_state": {
      push(value.service_name)
      push(value.phase ?? value.desired)
      push(value.health)
      break
    }
    case "instance": {
      push(value.service_name)
      push(value.event)
      break
    }
    default: {
      push(value.service_name)
      push(value.status)
      push(value.phase)
      push(value.event)
      push(value.transport)
      push(isRecord(value.scope) ? value.scope.type : null)
      if (typeof value.version === "number") push(`v${value.version}`)
    }
  }

  return tags.slice(0, 3)
}

function KvKeyDetailDialog({
  entry,
  onOpenChange,
}: {
  entry: KvKeyEntry | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const [viewMode, setViewMode] = useState<"tree" | "json">("tree")
  const parsed = entry ? parseCollectionDisplay(entry.collection) : null
  const fieldCount = entry ? Object.keys(entry.value).length : 0
  const subtitle = entry
    ? [
        parsed?.type?.replaceAll("_", " "),
        parsed?.layer,
        parsed?.namespace,
        `${fieldCount} ${t("kvFields").toLowerCase()}`,
      ]
        .filter(Boolean)
        .join(" · ")
    : undefined

  async function onCopy() {
    if (!entry) return
    await navigator.clipboard.writeText(JSON.stringify(entry.value, null, 2))
    toast.success(t("copy"))
  }

  return (
    <Dialog open={Boolean(entry)} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[54.4dvh] w-[min(90vw,calc(54.4dvh*1.618))]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-4 pr-12 @min-[640px]:px-5 @min-[640px]:pr-12">
          <DialogTitle className="truncate font-mono" title={entry?.key}>
            {entry?.key}
          </DialogTitle>
          {subtitle ? <DialogDescription className="capitalize">{subtitle}</DialogDescription> : null}
        </DialogHeader>
        {entry ? (
          <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
            <div className="flex min-h-0 flex-col gap-4 overflow-x-hidden overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
              <div className="grid gap-1">
                <div className="text-xs text-muted-foreground">{t("kvKey")}</div>
                <span className="break-all font-mono text-xs">{entry.key}</span>
              </div>
              {parsed?.type ? (
                <div className="grid gap-1">
                  <div className="text-xs text-muted-foreground">{t("kvType")}</div>
                  <span className="font-mono text-xs capitalize">
                    {parsed.type.replaceAll("_", " ")}
                  </span>
                </div>
              ) : null}
              {parsed?.layer ? (
                <div className="grid gap-1">
                  <div className="text-xs text-muted-foreground">{t("kvLayers")}</div>
                  <span className="font-mono text-xs">{parsed.layer}</span>
                </div>
              ) : null}
              {parsed?.namespace ? (
                <div className="grid gap-1">
                  <div className="text-xs text-muted-foreground">namespace</div>
                  <span className="font-mono text-xs">{parsed.namespace}</span>
                </div>
              ) : null}
              <div className="grid gap-1">
                <div className="text-xs text-muted-foreground">{t("kvFields")}</div>
                <span className="font-mono text-xs tabular-nums">{fieldCount}</span>
              </div>
            </div>
            <aside className="flex min-h-0 min-w-0 flex-col overflow-hidden max-h-[min(40dvh,20rem)] @min-[640px]:max-h-none">
              <Tabs
                value={viewMode}
                onValueChange={(value) => setViewMode(value as "tree" | "json")}
                className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden"
              >
                <div className="flex min-w-0 shrink-0 items-center gap-2">
                  <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
                    <TabsTrigger value="tree" className="flex-none">
                      Tree
                    </TabsTrigger>
                    <TabsTrigger value="json" className="flex-none">
                      JSON
                    </TabsTrigger>
                  </TabsList>
                  <Button
                    size="sm"
                    variant="outline"
                    className="ml-auto"
                    onClick={() => void onCopy()}
                  >
                    <ClipboardIcon data-icon="inline-start" />
                    {t("copy")}
                  </Button>
                </div>
                <TabsContent value="tree" className="mt-0 min-h-0 flex-1 overflow-hidden">
                  <ScrollPane className="h-full rounded-md border bg-muted/30 p-3">
                    <KvValueTree value={entry.value} />
                  </ScrollPane>
                </TabsContent>
                <TabsContent value="json" className="mt-0 min-h-0 flex-1 overflow-hidden">
                  <JsonBlock value={entry.value} className="h-full max-h-none" />
                </TabsContent>
              </Tabs>
            </aside>
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

export function KvKeysTable({
  layers,
  search = "",
}: {
  layers: KvLayerNode[]
  search?: string
}) {
  const { t } = useI18n()
  const [viewEntry, setViewEntry] = useState<KvKeyEntry | null>(null)
  const [pageSize, setPageSize] = useState<(typeof PAGE_SIZES)[number]>(20)
  const [page, setPage] = useState(0)
  const totalKeys = layers.reduce((sum, layer) => sum + layer.keyCount, 0)

  const rows = useMemo(() => {
    const all = layers.flatMap((layer) =>
      layer.collections.flatMap((collection: KvCollectionNode) =>
        collection.keys.map((entry) => ({
          entry,
          layer: layer.layer,
          type: collection.type,
        })),
      ),
    )
    if (!search.trim()) return all
    const query = search.toLowerCase()
    return all.filter(({ entry, layer, type }) => {
      const name = entryName(entry)
      const layerLabel = t(LAYER_I18N_KEY[layer]).toLowerCase()
      const tags = entryTags(entry, type).join(" ").toLowerCase()
      return (
        layerLabel.includes(query) ||
        type.toLowerCase().includes(query) ||
        name.toLowerCase().includes(query) ||
        entry.key.toLowerCase().includes(query) ||
        tags.includes(query)
      )
    })
  }, [layers, search, t])

  const total = rows.length
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const safePage = Math.min(page, pageCount - 1)
  const offset = safePage * pageSize
  const pageRows = rows.slice(offset, offset + pageSize)
  const from = total === 0 ? 0 : offset + 1
  const to = Math.min(offset + pageRows.length, total)

  useEffect(() => {
    setViewEntry(null)
  }, [layers])

  useEffect(() => {
    setPage(0)
  }, [search])

  function changePageSize(value: string) {
    setPageSize(Number(value) as (typeof PAGE_SIZES)[number])
    setPage(0)
  }

  if (totalKeys === 0) {
    return <p className="px-4 py-3 text-sm text-muted-foreground">{t("kvNoKeys")}</p>
  }

  const pager =
    total === 0 ? null : (
      <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t px-4 py-2">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>{t("gatewayFramesPageSize")}</span>
          <Select value={String(pageSize)} onValueChange={changePageSize}>
            <SelectTrigger size="sm" className="h-8 w-[72px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PAGE_SIZES.map((size) => (
                <SelectItem key={size} value={String(size)}>
                  {size}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span>{t("gatewayFramesShowing", { from, to, total })}</span>
        </div>
        <div className="flex items-center gap-1">
          <Button
            disabled={safePage <= 0}
            size="sm"
            variant="outline"
            onClick={() => setPage((prev) => Math.max(0, prev - 1))}
          >
            <ChevronLeftIcon className="size-3.5" />
            {t("gatewayFramesPrev")}
          </Button>
          <Button
            disabled={safePage + 1 >= pageCount}
            size="sm"
            variant="outline"
            onClick={() => setPage((prev) => prev + 1)}
          >
            {t("gatewayFramesNext")}
            <ChevronRightIcon className="size-3.5" />
          </Button>
        </div>
      </div>
    )

  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {total === 0 ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">{t("noResults")}</p>
        ) : (
          <>
            <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
              <Table className={TRACE_TABLE_CLASSNAME}>
                <TableHeader>
                  <TableRow>
                    <TableHead className={TRACE_COL1_CLASSNAME}>{t("kvLayers")}</TableHead>
                    <TableHead className={TRACE_COL2_CLASSNAME}>{t("kvKey")}</TableHead>
                    <TableHead>{t("gatewayFramesColTags")}</TableHead>
                    <TableHead>{t("kvValue")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {pageRows.map(({ entry, layer, type }) => {
                    const tags = entryTags(entry, type)
                    return (
                      <TableRow key={entry.id}>
                        <TableCell
                          className={cn(TRACE_COL1_CELL_CLASSNAME, "text-muted-foreground")}
                        >
                          {t(LAYER_I18N_KEY[layer])}
                        </TableCell>
                        <TableCell className={TRACE_COL2_CELL_CLASSNAME} title={entry.key}>
                          {entry.key}
                        </TableCell>
                        <TableCell>
                          {tags.length > 0 ? (
                            <div className={TRACE_COL_TAGS_CLASSNAME}>
                              {tags.map((tag, index) => (
                                <Badge
                                  key={`${entry.id}-${tag}`}
                                  variant={index === 0 ? "secondary" : "outline"}
                                  className="shrink-0 capitalize"
                                >
                                  {tag}
                                </Badge>
                              ))}
                            </div>
                          ) : (
                            <span className="text-xs text-muted-foreground">—</span>
                          )}
                        </TableCell>
                        <TableCell>
                          <Button
                            size="sm"
                            variant="link"
                            className="h-auto px-0 text-xs underline"
                            onClick={() => setViewEntry(entry)}
                          >
                            {t("kvValue")}
                          </Button>
                        </TableCell>
                      </TableRow>
                    )
                  })}
                </TableBody>
              </Table>
            </div>
            {pager}
          </>
        )}
      </div>
      <KvKeyDetailDialog
        entry={viewEntry}
        onOpenChange={(open) => {
          if (!open) setViewEntry(null)
        }}
      />
    </>
  )
}
