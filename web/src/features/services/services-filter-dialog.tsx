import { useState } from "react"
import { ListFilterIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import type { ServiceDisplayStatus } from "@/features/services/service-display-status"
import type {
  ServiceScopeFilter,
  ServiceSortBy,
  ServiceStatusFilter,
} from "@/features/services/use-services-list"
import {
  countActiveServiceFilters,
  SERVICE_STATUS_OPTIONS,
  toggleServiceStatusFilter,
} from "@/features/services/use-services-list"
import { useI18n } from "@/lib/i18n-context"
import { cn } from "@/lib/utils"

const STATUS_LABEL_KEYS: Record<ServiceDisplayStatus, string> = {
  connected: "filterConnected",
  connecting: "filterConnecting",
  disconnected: "filterDisconnected",
  error: "filterError",
}

export function ServicesFilterDialog(props: {
  activeFilterCount: number
  agentFilter?: string
  agentIds?: string[]
  hideScopeFilters?: boolean
  onAgentFilterChange?: (value: string) => void
  onScopeFilterChange?: (value: ServiceScopeFilter) => void
  onSort?: (value: ServiceSortBy) => void
  onStatusFilterChange: (value: ServiceStatusFilter) => void
  scopeFilter?: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [draftScope, setDraftScope] = useState<ServiceScopeFilter>(props.scopeFilter ?? "all")
  const [draftAgent, setDraftAgent] = useState(props.agentFilter ?? "")
  const [draftStatus, setDraftStatus] = useState<ServiceStatusFilter>(props.statusFilter)
  const [pendingSort, setPendingSort] = useState<ServiceSortBy | null>(null)

  const draftFilterCount = countActiveServiceFilters({
    scopeFilter: props.hideScopeFilters ? "all" : draftScope,
    statusFilter: draftStatus,
  })

  function beginDraft() {
    setDraftScope(props.scopeFilter ?? "all")
    setDraftAgent(props.agentFilter ?? "")
    setDraftStatus(props.statusFilter)
    setPendingSort(null)
  }

  function onOpenChange(next: boolean) {
    if (next) beginDraft()
    setOpen(next)
  }

  function confirm() {
    if (!props.hideScopeFilters) {
      props.onScopeFilterChange?.(draftScope)
      props.onAgentFilterChange?.(draftAgent)
    }
    props.onStatusFilterChange(draftStatus)
    if (pendingSort) props.onSort?.(pendingSort)
    setOpen(false)
  }

  function resetDraft() {
    setDraftScope("all")
    setDraftAgent("")
    setDraftStatus([])
    setPendingSort(null)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <ListFilterIcon data-icon="inline-start" />
          {t("filter")}
          {props.activeFilterCount > 0 ? (
            <Badge variant="secondary" className="h-5 min-w-5 px-1.5">
              {props.activeFilterCount}
            </Badge>
          ) : null}
        </Button>
      </DialogTrigger>
      <DialogContent
        className={cn(
          "@container flex max-h-none max-w-none flex-col gap-0 overflow-hidden p-0 sm:max-w-none",
          "h-[54.4dvh] w-[min(90vw,calc(54.4dvh*1.618))]",
        )}
      >
        <DialogHeader className="shrink-0 border-b px-4 py-4 @min-[640px]:px-5">
          <DialogTitle>{t("filter")}</DialogTitle>
          <DialogDescription>{t("filterDescription")}</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[auto_minmax(0,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
          <div className="min-h-0 overflow-x-hidden overflow-y-auto [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden @min-[640px]:h-full @min-[640px]:w-auto @min-[640px]:max-w-full @min-[640px]:aspect-[1/1.618]">
            <FieldGroup className="gap-6">
              {props.hideScopeFilters ? null : (
                <div className="grid gap-4">
                  <Field>
                    <FieldLabel>{t("scope")}</FieldLabel>
                    <Select
                      value={draftScope}
                      onValueChange={(value) => {
                        const next = value as ServiceScopeFilter
                        setDraftScope(next)
                        if (next !== "agent") setDraftAgent("")
                      }}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue placeholder={t("scope")} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          <SelectItem value="all">{t("filterAll")}</SelectItem>
                          <SelectItem value="store">{t("store")}</SelectItem>
                          <SelectItem value="agent">{t("agent")}</SelectItem>
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  </Field>
                  <Field>
                    <FieldLabel>{t("agent")}</FieldLabel>
                    <Select
                      value={draftAgent || "all"}
                      onValueChange={(value) => setDraftAgent(value === "all" ? "" : value)}
                      disabled={draftScope !== "agent"}
                    >
                      <SelectTrigger className="w-full">
                        <SelectValue placeholder={t("agent")} />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectGroup>
                          <SelectItem value="all">{t("filterAll")}</SelectItem>
                          {(props.agentIds || []).map((agentId) => (
                            <SelectItem key={agentId} value={agentId}>
                              {agentId}
                            </SelectItem>
                          ))}
                        </SelectGroup>
                      </SelectContent>
                    </Select>
                  </Field>
                </div>
              )}

              <FieldSet>
                <FieldLegend variant="label">{t("status")}</FieldLegend>
                <div data-slot="checkbox-group" className="flex flex-col gap-3">
                  {SERVICE_STATUS_OPTIONS.map((status) => {
                    const id = `service-status-${status}`
                    const checked = draftStatus.includes(status)
                    return (
                      <Field key={status} orientation="horizontal" className="items-center gap-2">
                        <Checkbox
                          id={id}
                          checked={checked}
                          onCheckedChange={() =>
                            setDraftStatus((current) => toggleServiceStatusFilter(current, status))
                          }
                        />
                        <Label htmlFor={id} className="font-normal">
                          {t(STATUS_LABEL_KEYS[status])}
                        </Label>
                      </Field>
                    )
                  })}
                </div>
              </FieldSet>
            </FieldGroup>
          </div>

          <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none">
            {props.onSort ? (
              <Field className="min-h-0 flex-1">
                <FieldLabel>{t("sortLabel")}</FieldLabel>
                <div className="mt-2 grid grid-cols-3 gap-2">
                  <Button
                    variant={pendingSort === "name" ? "default" : "outline"}
                    size="sm"
                    onClick={() => setPendingSort("name")}
                  >
                    {t("sortName")}
                  </Button>
                  <Button
                    variant={pendingSort === "status" ? "default" : "outline"}
                    size="sm"
                    onClick={() => setPendingSort("status")}
                  >
                    {t("sortStatus")}
                  </Button>
                  <Button
                    variant={pendingSort === "tools" ? "default" : "outline"}
                    size="sm"
                    onClick={() => setPendingSort("tools")}
                  >
                    {t("sortTools")}
                  </Button>
                </div>
                <FieldDescription className="mt-2">{t("sortHint")}</FieldDescription>
              </Field>
            ) : (
              <div className="text-sm text-muted-foreground">{t("filterDescription")}</div>
            )}
          </aside>
        </div>

        <div className="shrink-0 border-t px-5 py-3.5">
          <DialogFooter className={draftFilterCount > 0 ? "sm:justify-between" : "sm:justify-end"}>
            {draftFilterCount > 0 ? (
              <Button type="button" variant="ghost" onClick={resetDraft}>
                {t("reset")}
              </Button>
            ) : null}
            <div className="flex flex-col-reverse gap-2 sm:flex-row">
              <DialogClose asChild>
                <Button type="button" variant="outline">
                  {t("cancel")}
                </Button>
              </DialogClose>
              <Button type="button" onClick={confirm}>
                {t("confirm")}
              </Button>
            </div>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  )
}
