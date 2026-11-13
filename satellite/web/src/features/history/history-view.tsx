import { useState } from "react"
import { useQuery } from "@tanstack/react-query"
import { ChevronLeftIcon, ChevronRightIcon } from "lucide-react"

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
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import {
  TRACE_COL1_CELL_CLASSNAME,
  TRACE_COL1_CLASSNAME,
  TRACE_COL2_CELL_CLASSNAME,
  TRACE_COL2_CLASSNAME,
  TRACE_TABLE_CLASSNAME,
} from "@/features/traces/trace-table-layout"
import { useI18n } from "@/lib/i18n-context"
import { queryKeys } from "@/lib/query-keys"
import { cn } from "@/lib/utils"
import { getRequestHistory, type RequestRecord } from "@/lib/api"

const PAGE_SIZES = [10, 20, 50] as const

function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
}

/** Request-history table body for embedding (e.g. Traces tabs). */
export function HistoryPanel() {
  const { t } = useI18n()
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
      {total === 0 && !history.isLoading ? (
        <p className="px-4 py-3 text-sm text-muted-foreground">{t("gatewayHistoryEmpty")}</p>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
          <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
            <Table className={TRACE_TABLE_CLASSNAME}>
              <TableHeader>
                <TableRow>
                  <TableHead className={TRACE_COL1_CLASSNAME}>
                    {t("gatewayHistoryColTime")}
                  </TableHead>
                  <TableHead className={TRACE_COL2_CLASSNAME}>
                    {t("gatewayHistoryColTarget")}
                  </TableHead>
                  <TableHead>{t("gatewayHistoryColCaller")}</TableHead>
                  <TableHead>{t("gatewayHistoryColKind")}</TableHead>
                  <TableHead>{t("gatewayHistoryColStatus")}</TableHead>
                  <TableHead>{t("gatewayHistoryColDuration")}</TableHead>
                  <TableHead>{t("gatewayFramesColActions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {records.map((record) => {
                  const fullTime = new Date(record.started_at_unix_ms).toLocaleString()
                  return (
                    <TableRow key={record.id}>
                      <TableCell
                        className={cn(TRACE_COL1_CELL_CLASSNAME, "text-muted-foreground")}
                      >
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <span className="cursor-default">
                              {formatClock(record.started_at_unix_ms)}
                            </span>
                          </TooltipTrigger>
                          <TooltipContent side="top" sideOffset={6}>
                            {fullTime}
                          </TooltipContent>
                        </Tooltip>
                      </TableCell>
                      <TableCell
                        className={TRACE_COL2_CELL_CLASSNAME}
                        title={targetLabel(record)}
                      >
                        {targetLabel(record)}
                      </TableCell>
                      <TableCell className="max-w-[10rem] truncate font-mono text-xs">
                        {record.caller ?? t("gatewayCallDirect")}
                      </TableCell>
                      <TableCell className="text-xs text-muted-foreground">
                        {record.kind === "tool"
                          ? t("gatewayKindTool")
                          : t("gatewayKindManagement")}
                      </TableCell>
                      <TableCell>
                        <Badge variant={record.success ? "secondary" : "destructive"}>
                          {record.success ? "OK" : "ERR"}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-xs tabular-nums text-muted-foreground">
                        {record.duration_ms}ms
                      </TableCell>
                      <TableCell>
                        <Button
                          size="sm"
                          variant="link"
                          className="h-auto px-0 text-xs underline"
                          onClick={() => setActive(record)}
                        >
                          {t("detail")}
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          </div>
          {pager}
        </div>
      )}

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

/** Full CLI argv from audit payload; `tool` is capped at 200 chars for list display. */
function argvFromArgs(argsJson: string | null): string[] | null {
  if (!argsJson) return null
  try {
    const parsed = JSON.parse(argsJson) as { argv?: unknown }
    if (
      Array.isArray(parsed.argv) &&
      parsed.argv.length > 0 &&
      parsed.argv.every((item) => typeof item === "string")
    ) {
      return parsed.argv as string[]
    }
  } catch {
    // non-JSON / unexpected shape
  }
  return null
}

function detailTargetTokens(record: RequestRecord): string[] | null {
  if (record.kind === "tool") return null
  const argv = argvFromArgs(record.args_json)
  return argv ? ["cli", ...argv] : null
}

function detailTargetLabel(record: RequestRecord): string {
  const tokens = detailTargetTokens(record)
  if (tokens) return tokens.join(" ")
  return targetLabel(record)
}

/** Short verb-like token (binary / subcommand), not uuid/path/value. */
function isCommandWord(token: string): boolean {
  return /^[A-Za-z][\w-]*$/.test(token) && token.length <= 32 && !token.includes("/")
}

function looksLikeCommand(text: string): boolean {
  return /\s--?[A-Za-z]/.test(text) || /^(cli|mcpstore)\b/i.test(text.trim())
}

function splitCommandTokens(input: string): string[] {
  return input.trim().match(/(?:[^\s"']+|"[^"]*"|'[^']*')+/g) ?? []
}

type CommandPart =
  | { kind: "head"; text: string }
  | { kind: "positional"; text: string }
  | { kind: "flag"; flag: string; value?: string }

function parseCommandPartsFromTokens(tokens: string[]): CommandPart[] {
  if (tokens.length === 0) return []

  const parts: CommandPart[] = []
  let i = 0
  const head: string[] = []
  while (i < tokens.length && !tokens[i]!.startsWith("-") && isCommandWord(tokens[i]!)) {
    head.push(tokens[i]!)
    i += 1
  }
  if (head.length > 0) parts.push({ kind: "head", text: head.join(" ") })

  while (i < tokens.length) {
    const tok = tokens[i]!
    if (tok.startsWith("-")) {
      const eq = tok.match(/^([^=]+)=(.*)$/)
      if (eq) {
        parts.push({ kind: "flag", flag: eq[1]!, value: eq[2] })
        i += 1
        continue
      }
      const next = tokens[i + 1]
      if (next && !next.startsWith("-")) {
        parts.push({ kind: "flag", flag: tok, value: next })
        i += 2
      } else {
        parts.push({ kind: "flag", flag: tok })
        i += 1
      }
      continue
    }
    parts.push({ kind: "positional", text: tok })
    i += 1
  }
  return parts
}

function FormattedTarget({ text, tokens }: { text: string; tokens?: string[] | null }) {
  const parts =
    tokens && tokens.length > 0
      ? parseCommandPartsFromTokens(tokens)
      : looksLikeCommand(text)
        ? parseCommandPartsFromTokens(splitCommandTokens(text))
        : []
  if (parts.length <= 1) {
    return <span className="break-all font-mono text-xs">{text}</span>
  }
  return (
    <div className="flex flex-col gap-1 font-mono text-xs">
      {parts.map((part, index) => {
        if (part.kind === "head") {
          return (
            <div key={index} className="font-medium text-foreground">
              {part.text}
            </div>
          )
        }
        if (part.kind === "positional") {
          return (
            <div key={index} className="break-all pl-3 text-foreground/90">
              {part.text}
            </div>
          )
        }
        return (
          <div key={index} className="break-all pl-3">
            <span className="text-muted-foreground">{part.flag}</span>
            {part.value != null && part.value !== "" ? (
              <span className="text-foreground/90"> {part.value}</span>
            ) : null}
          </div>
        )
      })}
    </div>
  )
}

function RequestDetailDialog({
  record,
  onOpenChange,
}: {
  record: RequestRecord | null
  onOpenChange: (open: boolean) => void
}) {
  const { t } = useI18n()
  const kindLabel =
    record?.kind === "tool" ? t("gatewayKindTool") : t("gatewayKindManagement")
  const subtitle = record
    ? [
        new Date(record.started_at_unix_ms).toLocaleString(),
        record.caller ?? t("gatewayCallDirect"),
        kindLabel,
        record.success ? "OK" : "ERR",
        `${record.duration_ms}ms`,
      ].join(" · ")
    : undefined

  return (
    <Dialog open={record !== null} onOpenChange={onOpenChange}>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[54.4dvh] w-[min(90vw,calc(54.4dvh*1.618))]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-4 pr-12 @min-[640px]:px-5 @min-[640px]:pr-12">
          <DialogTitle className="truncate font-mono">
            {t("gatewayHistoryDialogTitle", {
              id: record ? targetLabel(record) : "",
            })}
          </DialogTitle>
          {subtitle ? <DialogDescription>{subtitle}</DialogDescription> : null}
        </DialogHeader>
        {record ? (
          <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[minmax(0,0.9fr)_minmax(0,1.4fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
            <div className="flex min-h-0 flex-col overflow-x-hidden overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
              <div className="grid gap-1">
                <div className="text-xs text-muted-foreground">{t("gatewayHistoryColTarget")}</div>
                <FormattedTarget
                  text={detailTargetLabel(record)}
                  tokens={detailTargetTokens(record)}
                />
              </div>
            </div>
            <aside className="flex min-h-0 min-w-0 flex-col overflow-hidden max-h-[min(40dvh,20rem)] @min-[640px]:max-h-none">
              <DetailPayloadTabs record={record} />
            </aside>
          </div>
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

function prettyJson(raw: string | null | undefined): string | null {
  if (!raw) return null
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

function DetailPayloadTabs({ record }: { record: RequestRecord }) {
  const { t } = useI18n()
  const hasError = Boolean(record.error_summary)
  const defaultTab = hasError ? "error" : "args"

  return (
    <Tabs
      key={record.id}
      defaultValue={defaultTab}
      className="flex min-h-0 flex-1 flex-col gap-2 overflow-hidden"
    >
      <div className="flex min-w-0 shrink-0 items-center gap-2">
        <TabsList variant="line" className="h-9 w-fit shrink-0 justify-start">
          <TabsTrigger value="args" className="flex-none">
            {t("gatewayDetailArgs")}
          </TabsTrigger>
          <TabsTrigger value="result" className="flex-none">
            {t("gatewayDetailResult")}
          </TabsTrigger>
          {hasError ? (
            <TabsTrigger value="error" className="flex-none text-destructive">
              {t("error")}
            </TabsTrigger>
          ) : null}
        </TabsList>
        {record.args_truncated || record.result_truncated ? (
          <Badge variant="outline" className="ml-auto text-[10px]">
            {t("gatewayTruncated")}
          </Badge>
        ) : null}
      </div>

      <TabsContent value="args" className="mt-0 min-h-0 flex-1 overflow-hidden">
        <PayloadPre text={prettyJson(record.args_json)} />
      </TabsContent>
      <TabsContent value="result" className="mt-0 min-h-0 flex-1 overflow-hidden">
        <PayloadPre text={prettyJson(record.result_json)} />
      </TabsContent>
      {hasError ? (
        <TabsContent value="error" className="mt-0 min-h-0 flex-1 overflow-hidden">
          <div className="flex h-full min-h-0 flex-col gap-2 overflow-hidden">
            {record.error_code ? (
              <Badge variant="destructive" className="w-fit shrink-0">
                {record.error_code}
              </Badge>
            ) : null}
            <PayloadPre text={prettyJson(record.error_summary)} />
          </div>
        </TabsContent>
      ) : null}
    </Tabs>
  )
}

function PayloadPre({ text }: { text: string | null }) {
  if (!text) {
    return <p className="text-sm text-muted-foreground">—</p>
  }
  return (
    <pre className="h-full min-h-0 overflow-auto whitespace-pre-wrap break-all rounded-md bg-muted/50 p-2 font-mono text-xs [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
      {text}
    </pre>
  )
}
