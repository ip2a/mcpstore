import { Badge } from "@/components/ui/badge"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useGatewayRequestFlow } from "@/features/gateway/use-gateway-request-flow"
import {
  TRACE_COL1_CELL_CLASSNAME,
  TRACE_COL1_CLASSNAME,
  TRACE_COL2_CELL_CLASSNAME,
  TRACE_COL2_CLASSNAME,
  TRACE_TABLE_CLASSNAME,
} from "@/features/traces/trace-table-layout"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"
import type { GatewayRequestRecord } from "@/lib/api"

const KIND_LABEL_KEYS = {
  tool: "gatewayCallKindTool",
  prompt: "gatewayCallKindPrompt",
  resource: "gatewayCallKindResource",
} as const

function formatClock(ms: number): string {
  return new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
}

function targetLabel(record: GatewayRequestRecord): string {
  return record.kind === "resource" ? record.name : `${record.service_name}.${record.name}`
}

/** Live gateway routed-calls table for embedding (Traces → Routed calls tab). */
export function RoutedCallsPanel() {
  const { t } = useI18n()
  const records = useGatewayRequestFlow()
  const rows = [...records].reverse()

  if (rows.length === 0) {
    return (
      <p className="px-4 py-3 text-sm text-muted-foreground">{t("gatewayRequestsEmpty")}</p>
    )
  }

  return (
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
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((record) => {
              const fullTime = new Date(record.at_unix_ms).toLocaleString()
              const target = targetLabel(record)
              return (
                <TableRow key={record.seq}>
                  <TableCell
                    className={cn(TRACE_COL1_CELL_CLASSNAME, "text-muted-foreground")}
                  >
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <span className="cursor-default">{formatClock(record.at_unix_ms)}</span>
                      </TooltipTrigger>
                      <TooltipContent side="top" sideOffset={6}>
                        {fullTime}
                      </TooltipContent>
                    </Tooltip>
                  </TableCell>
                  <TableCell className={TRACE_COL2_CELL_CLASSNAME} title={target}>
                    {target}
                  </TableCell>
                  <TableCell className="max-w-[10rem] truncate font-mono text-xs">
                    {record.caller ?? t("gatewayCallDirect")}
                  </TableCell>
                  <TableCell className="text-xs text-muted-foreground">
                    {t(KIND_LABEL_KEYS[record.kind])}
                  </TableCell>
                  <TableCell>
                    <Badge variant={record.success ? "secondary" : "destructive"}>
                      {record.success ? "OK" : "ERR"}
                    </Badge>
                  </TableCell>
                  <TableCell className="text-xs tabular-nums text-muted-foreground">
                    {record.duration_ms}ms
                  </TableCell>
                </TableRow>
              )
            })}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
