import { useEffect, useMemo, useState } from "react"
import { ClipboardIcon, SearchIcon } from "lucide-react"
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
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
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
import { useI18n } from "@/lib/i18n-context"

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
  return fieldToText(entry.value.name) ?? entry.key
}

function entrySummary(entry: KvKeyEntry): string | null {
  return (
    fieldToText(entry.value.description) ??
    fieldToText(entry.value.status) ??
    fieldToText(entry.value.type) ??
    fieldToText(entry.value.state) ??
    null
  )
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
              {entry ? <KvValueTree value={entry.value} /> : null}
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

export function KvKeysTable({ layers }: { layers: KvLayerNode[] }) {
  const { t } = useI18n()
  const [viewEntry, setViewEntry] = useState<KvKeyEntry | null>(null)
  const [search, setSearch] = useState("")
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
      return (
        layerLabel.includes(query) ||
        type.toLowerCase().includes(query) ||
        name.toLowerCase().includes(query) ||
        entry.key.toLowerCase().includes(query)
      )
    })
  }, [layers, search, t])

  useEffect(() => {
    setViewEntry(null)
    setSearch("")
  }, [layers])

  if (totalKeys === 0) {
    return <p className="px-1 py-3 text-sm text-muted-foreground">{t("kvNoKeys")}</p>
  }

  return (
    <>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {totalKeys > 4 ? (
          <div className="relative mb-2 shrink-0">
            <SearchIcon className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              type="search"
              placeholder={t("searchKeys")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-9 pl-8"
            />
          </div>
        ) : null}
        {rows.length === 0 ? (
          <p className="px-1 py-3 text-sm text-muted-foreground">{t("noResults")}</p>
        ) : (
          <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>{t("kvLayers")}</TableHead>
                  <TableHead>{t("kvType")}</TableHead>
                  <TableHead>{t("name")}</TableHead>
                  <TableHead>{t("kvKey")}</TableHead>
                  <TableHead>{t("kvFields")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map(({ entry, layer, type }) => {
                  const name = entryName(entry)
                  const summary = entrySummary(entry)
                  const fieldCount = Object.keys(entry.value).length
                  return (
                    <TableRow
                      key={entry.id}
                      className="cursor-pointer"
                      onClick={() => setViewEntry(entry)}
                    >
                      <TableCell>
                        <Badge variant="outline">{t(LAYER_I18N_KEY[layer])}</Badge>
                      </TableCell>
                      <TableCell>
                        <Badge variant="secondary" className="capitalize">
                          {type.replaceAll("_", " ")}
                        </Badge>
                      </TableCell>
                      <TableCell className="max-w-[220px]">
                        <div className="min-w-0 truncate text-sm font-medium" title={name}>
                          {name}
                        </div>
                        {summary && summary !== name ? (
                          <div
                            className="mt-0.5 truncate text-xs text-muted-foreground"
                            title={summary}
                          >
                            {summary}
                          </div>
                        ) : null}
                      </TableCell>
                      <TableCell
                        className="max-w-[280px] truncate font-mono text-xs text-muted-foreground"
                        title={entry.key}
                      >
                        {entry.key}
                      </TableCell>
                      <TableCell className="tabular-nums text-xs text-muted-foreground">
                        {fieldCount}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </div>
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
