import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { useGatewayProjectionQuery, useGatewayStatusQuery } from "@/features/clients/queries"
import {
  TRACE_COL1_CELL_CLASSNAME,
  TRACE_COL1_CLASSNAME,
  TRACE_COL2_CELL_CLASSNAME,
  TRACE_COL2_CLASSNAME,
  TRACE_TABLE_CLASSNAME,
} from "@/features/traces/trace-table-layout"
import { useI18n } from "@/lib/i18n-context"
import type { GatewayProjectionIssue } from "@/lib/api"

type ProjectionRow = {
  key: string
  mount: string
  kind: "down" | "skipped" | "degraded"
  service: string
  tool: string
  reason: string
}

function flattenIssues(
  mounts: { caller: string | null; issues: GatewayProjectionIssue[] }[],
  genericMount: string,
): ProjectionRow[] {
  const rows: ProjectionRow[] = []
  for (const mount of mounts) {
    const mountLabel = mount.caller ?? genericMount
    mount.issues.forEach((issue, index) => {
      if ("Down" in issue) {
        rows.push({
          key: `${mountLabel}:down:${issue.Down.service}:${index}`,
          mount: mountLabel,
          kind: "down",
          service: issue.Down.service,
          tool: "—",
          reason: issue.Down.reason,
        })
        return
      }
      if ("SkippedTool" in issue) {
        rows.push({
          key: `${mountLabel}:skipped:${issue.SkippedTool.service}:${issue.SkippedTool.tool}:${index}`,
          mount: mountLabel,
          kind: "skipped",
          service: issue.SkippedTool.service ?? "—",
          tool: issue.SkippedTool.tool ?? "—",
          reason: issue.SkippedTool.reason,
        })
        return
      }
      rows.push({
        key: `${mountLabel}:degraded:${issue.DegradedTool.service}:${issue.DegradedTool.tool}:${index}`,
        mount: mountLabel,
        kind: "degraded",
        service: issue.DegradedTool.service,
        tool: issue.DegradedTool.tool,
        reason: "—",
      })
    })
  }
  return rows
}

/** Gateway projection defects table (Traces → Projection tab). */
export function ProjectionPanel() {
  const { t } = useI18n()
  const status = useGatewayStatusQuery()
  const projection = useGatewayProjectionQuery(status.data?.running === true)
  const rows = flattenIssues(projection.data?.mounts ?? [], t("gatewayProjectionGenericMount"))

  if (rows.length === 0) {
    return <p className="px-4 py-3 text-sm text-muted-foreground">{t("gatewayProjectionEmpty")}</p>
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="min-h-0 flex-1 overflow-x-hidden overflow-y-auto px-4 [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden">
        <Table className={TRACE_TABLE_CLASSNAME}>
          <TableHeader>
            <TableRow>
              <TableHead className={TRACE_COL1_CLASSNAME}>
                {t("gatewayProjectionColKind")}
              </TableHead>
              <TableHead className={TRACE_COL2_CLASSNAME}>
                {t("gatewayProjectionColReason")}
              </TableHead>
              <TableHead>{t("gatewayProjectionColMount")}</TableHead>
              <TableHead>{t("gatewayProjectionColTool")}</TableHead>
              <TableHead>{t("gatewayProjectionColService")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {rows.map((row) => (
              <TableRow key={row.key}>
                <TableCell className={TRACE_COL1_CELL_CLASSNAME}>
                  {row.kind === "down"
                    ? t("gatewayProjectionIssueDown")
                    : row.kind === "skipped"
                      ? t("gatewayProjectionIssueSkipped")
                      : t("gatewayProjectionIssueDegraded")}
                </TableCell>
                <TableCell
                  className={`${TRACE_COL2_CELL_CLASSNAME} text-xs text-muted-foreground`}
                  title={row.reason}
                >
                  <span className="line-clamp-2">{row.reason}</span>
                </TableCell>
                <TableCell title={row.mount}>{row.mount}</TableCell>
                <TableCell className="max-w-[10rem] truncate font-mono text-xs" title={row.tool}>
                  {row.tool}
                </TableCell>
                <TableCell title={row.service}>{row.service}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  )
}
