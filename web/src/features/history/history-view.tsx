import { useState } from "react"
import { useQuery, useQueryClient } from "@tanstack/react-query"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

import { PanelCard } from "@/components/shared/panel-card"
import { RefreshCwSpinIcon } from "@/components/shared/refresh-cw-spin-icon"
import { WorkspacePageHeader } from "@/components/shared/workspace-page-header"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"
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
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"
import { getRequestHistory, type RequestRecord } from "@/lib/api"

const PAGE_SIZES = [10, 20, 50] as const

/** Insights → History: daemon telemetry request log (tools + management). */
export function HistoryView() {
  const { t } = useI18n()
  const queryClient = useQueryClient()
  const [pageSize, setPageSize] = useState<(typeof PAGE_SIZES)[number]>(20)
  const [page, setPage] = useState(0)
  const [active, setActive] = useState<RequestRecord | null>(null)

  const offset = page * pageSize
  const history = useQuery({
    queryKey: queryKeys.gatewayHistoryPage(pageSize, offset),
    queryFn: () => getRequestHistory({ limit: pageSize, offset }),
    refetchInterval: 15_000,
  })

  const records = history.data?.requests ?? []
  const total = history.data?.total ?? 0
  const pageCount = Math.max(1, Math.ceil(total / pageSize))
  const from = total === 0 ? 0 : offset + 1
  const to = Math.min(offset + records.length, total)

  function refresh() {
    void queryClient.invalidateQueries({ queryKey: queryKeys.gatewayHistory })
  }

  function changePageSize(value: string) {
    setPageSize(Number(value) as (typeof PAGE_SIZES)[number])
    setPage(0)
  }

  const pager =
    total === 0 && !history.isLoading ? null : (
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
            disabled={page <= 0}
            size="sm"
            variant="outline"
            onClick={() => setPage((prev) => Math.max(0, prev - 1))}
          >
            <ChevronLeftIcon className="size-3.5" />
            {t("gatewayFramesPrev")}
          </Button>
          <Button
            disabled={page + 1 >= pageCount}
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
      <WorkspacePageHeader
        title={<h1 className="text-lg font-semibold">{t("historyWorkspaceTitle")}</h1>}
        subtitle={t("historyWorkspaceDescription", { count: total })}
      />

      <PanelCard className="h-full min-h-0 flex-1 gap-0 p-0" variant="stack">
        <div className="flex w-full min-w-0 shrink-0 items-center gap-3 border-b px-4 py-2">
          <div className="text-sm font-medium">{t("gatewayHistoryTitle")}</div>
          <div className="ml-auto flex shrink-0 items-center gap-2">
            <Button disabled={history.isFetching} size="sm" variant="outline" onClick={refresh}>
              <RefreshCwSpinIcon data-icon="inline-start" spinning={history.isFetching} />
              {t("refresh")}
            </Button>
          </div>
        </div>

        {total === 0 && !history.isLoading ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">{t("gatewayHistoryEmpty")}</p>
        ) : (
          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("gatewayHistoryColTime")}</TableHead>
                    <TableHead>{t("gatewayHistoryColCaller")}</TableHead>
                    <TableHead>{t("gatewayHistoryColTarget")}</TableHead>
                    <TableHead>{t("gatewayHistoryColKind")}</TableHead>
                    <TableHead>{t("gatewayHistoryColStatus")}</TableHead>
                    <TableHead className="text-right">{t("gatewayHistoryColDuration")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {records.map((record) => (
                    <TableRow
                      key={record.id}
                      className="cursor-pointer"
                      onClick={() => setActive(record)}
                    >
                      <TableCell className="whitespace-nowrap tabular-nums text-xs text-muted-foreground">
                        {new Date(record.started_at_unix_ms).toLocaleString()}
                      </TableCell>
                      <TableCell className="max-w-[140px] truncate font-mono text-xs">
                        {record.caller ?? t("gatewayCallDirect")}
                      </TableCell>
                      <TableCell
                        className="max-w-[280px] truncate font-mono text-xs"
                        title={targetLabel(record)}
                      >
                        {targetLabel(record)}
                      </TableCell>
                      <TableCell>
                        <Badge variant="secondary">
                          {record.kind === "tool"
                            ? t("gatewayKindTool")
                            : t("gatewayKindManagement")}
                        </Badge>
                      </TableCell>
                      <TableCell>
                        <Badge variant={record.success ? "secondary" : "destructive"}>
                          {record.success ? "OK" : "ERR"}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right tabular-nums text-xs text-muted-foreground">
                        {record.duration_ms}ms
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
            {pager}
          </div>
        )}
      </PanelCard>

      <RequestDetailDialog
        onOpenChange={(open) => {
          if (!open) setActive(null)
        }}
        record={active}
      />
    </>
  )
}

function targetLabel(record: RequestRecord): string {
  if (record.kind === "tool") {
    return `${record.service_name ?? "—"}.${record.tool ?? "—"}`
  }
  return record.tool ?? "—"
}

function RequestDetailDialog({
  record,
  onOpenChange,
}: {
  record: RequestRecord | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  return (
    <Dialog open={record !== null} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[min(85vh,720px)] flex-col gap-3 overflow-hidden sm:max-w-2xl">
        <DialogHeader className="shrink-0">
          <DialogTitle className="truncate font-mono text-base">
            {t("gatewayHistoryDialogTitle", {
              id: record ? targetLabel(record) : "",
            })}
          </DialogTitle>
        </DialogHeader>
        {record ? (
          <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto text-xs">
            <div className="flex flex-wrap items-center gap-2">
              <span className="tabular-nums text-muted-foreground">
                {new Date(record.started_at_unix_ms).toLocaleString()}
              </span>
              <Badge variant="outline" className="font-mono">
                {record.caller ?? t("gatewayCallDirect")}
              </Badge>
              <Badge variant="secondary">
                {record.kind === "tool" ? t("gatewayKindTool") : t("gatewayKindManagement")}
              </Badge>
              <Badge variant={record.success ? "secondary" : "destructive"}>
                {record.success ? "OK" : "ERR"}
              </Badge>
              <span className="text-muted-foreground">{record.duration_ms}ms</span>
            </div>
            {record.error_summary ? (
              <div>
                <span className="font-medium text-destructive">{record.error_code ?? "ERR"}</span>
                <pre className="mt-1 max-h-32 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2">
                  {record.error_summary}
                </pre>
              </div>
            ) : null}
            <PayloadBlock
              title={t("gatewayDetailArgs")}
              payload={record.args_json}
              truncated={record.args_truncated}
            />
            <PayloadBlock
              title={t("gatewayDetailResult")}
              payload={record.result_json}
              truncated={record.result_truncated}
            />
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

function PayloadBlock({
  title,
  payload,
  truncated,
}: {
  title: string
  payload: string | null
  truncated: boolean
}) {
  const { t } = useI18n()
  if (!payload) return null
  let pretty = payload
  try {
    pretty = JSON.stringify(JSON.parse(payload), null, 2)
  } catch {
    // non-JSON (truncated residue) shown as-is
  }
  return (
    <div className="min-w-0">
      <div className="flex items-center gap-2">
        <span className="font-medium">{title}</span>
        {truncated ? (
          <Badge variant="outline" className="text-[10px]">
            {t("gatewayTruncated")}
          </Badge>
        ) : null}
      </div>
      <pre
        className={cn(
          "mt-1 max-h-64 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/50 p-2",
        )}
      >
        {pretty}
      </pre>
    </div>
  )
}
