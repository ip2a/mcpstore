import { useState } from "react"
import { ChevronDownIcon, ChevronUpIcon, ListFilterIcon } from "lucide-react"

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
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field"
import { Label } from "@/components/ui/label"
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { ServiceDisplayStatus } from "@/features/services/service-display-status"
import type {
  ServicePrimitiveKind,
  ServiceScopeFilter,
  ServiceSortBy,
  ServiceSortDirection,
  ServiceSortSpec,
  ServiceStatusFilter,
} from "@/features/services/use-services-list"
import {
  countActiveServiceFilters,
  DEFAULT_STATUS_SORT_PRIORITY,
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

function movePriority(
  priority: ServiceDisplayStatus[],
  index: number,
  direction: -1 | 1,
): ServiceDisplayStatus[] {
  const nextIndex = index + direction
  if (nextIndex < 0 || nextIndex >= priority.length) return priority
  const next = [...priority]
  ;[next[index], next[nextIndex]] = [next[nextIndex], next[index]]
  return next
}

export function ServicesFilterDialog(props: {
  activeFilterCount: number
  agentFilter?: string
  agentIds?: string[]
  hideScopeFilters?: boolean
  onAgentFilterChange?: (value: string) => void
  onScopeFilterChange?: (value: ServiceScopeFilter) => void
  onSort?: (value: ServiceSortSpec) => void
  onStatusFilterChange: (value: ServiceStatusFilter) => void
  scopeFilter?: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [draftScope, setDraftScope] = useState<ServiceScopeFilter>(props.scopeFilter ?? "all")
  const [draftAgent, setDraftAgent] = useState(props.agentFilter ?? "")
  const [draftStatus, setDraftStatus] = useState<ServiceStatusFilter>(props.statusFilter)
  const [sortTab, setSortTab] = useState<ServiceSortBy>("name")
  const [nameDirection, setNameDirection] = useState<ServiceSortDirection>("asc")
  const [primitiveKind, setPrimitiveKind] = useState<ServicePrimitiveKind>("tools")
  const [primitiveDirection, setPrimitiveDirection] = useState<ServiceSortDirection>("desc")
  const [statusPriority, setStatusPriority] = useState<ServiceDisplayStatus[]>(
    DEFAULT_STATUS_SORT_PRIORITY,
  )
  const [pendingSort, setPendingSort] = useState<ServiceSortSpec | null>(null)

  const draftFilterCount = countActiveServiceFilters({
    scopeFilter: props.hideScopeFilters ? "all" : draftScope,
    statusFilter: draftStatus,
  })

  function beginDraft() {
    setDraftScope(props.scopeFilter ?? "all")
    setDraftAgent(props.agentFilter ?? "")
    setDraftStatus(props.statusFilter)
    setSortTab("name")
    setNameDirection("asc")
    setPrimitiveKind("tools")
    setPrimitiveDirection("desc")
    setStatusPriority(DEFAULT_STATUS_SORT_PRIORITY)
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

  function selectNameDirection(direction: ServiceSortDirection) {
    setNameDirection(direction)
    setPendingSort({ by: "name", direction })
  }

  function selectPrimitive(kind: ServicePrimitiveKind, direction: ServiceSortDirection) {
    setPrimitiveKind(kind)
    setPrimitiveDirection(direction)
    setPendingSort({ by: "primitives", kind, direction })
  }

  function changeStatusPriority(index: number, direction: -1 | 1) {
    setStatusPriority((current) => {
      const next = movePriority(current, index, direction)
      setPendingSort({ by: "status", priority: next })
      return next
    })
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
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <Tabs
                  value={sortTab}
                  onValueChange={(value) => {
                    const tab = value as ServiceSortBy
                    setSortTab(tab)
                    if (tab === "status") {
                      setPendingSort({ by: "status", priority: statusPriority })
                    }
                  }}
                  className="flex min-h-0 flex-1 flex-col gap-3"
                >
                  <div className="flex min-w-0 shrink-0 items-center gap-3">
                    <FieldLabel className="shrink-0">{t("sortLabel")}</FieldLabel>
                    <TabsList
                      variant="line"
                      className="h-9 min-w-0 flex-1 justify-start overflow-x-auto [scrollbar-width:none]"
                    >
                      <TabsTrigger value="name" className="flex-none">
                        {t("sortName")}
                      </TabsTrigger>
                      <TabsTrigger value="status" className="flex-none">
                        {t("sortStatus")}
                      </TabsTrigger>
                      <TabsTrigger value="primitives" className="flex-none">
                        {t("sortPrimitives")}
                      </TabsTrigger>
                    </TabsList>
                  </div>

                  <TabsContent value="name" className="mt-0 flex min-h-0 flex-1 flex-col gap-3">
                    <RadioGroup
                      value={pendingSort?.by === "name" ? nameDirection : undefined}
                      onValueChange={(value) =>
                        selectNameDirection(value as ServiceSortDirection)
                      }
                      className="flex flex-col gap-3"
                    >
                      <Field orientation="horizontal" className="items-center gap-2">
                        <RadioGroupItem value="asc" id="sort-name-asc" />
                        <Label htmlFor="sort-name-asc" className="font-normal">
                          {t("sortNameAsc")}
                        </Label>
                      </Field>
                      <Field orientation="horizontal" className="items-center gap-2">
                        <RadioGroupItem value="desc" id="sort-name-desc" />
                        <Label htmlFor="sort-name-desc" className="font-normal">
                          {t("sortNameDesc")}
                        </Label>
                      </Field>
                    </RadioGroup>
                  </TabsContent>

                  <TabsContent value="status" className="mt-0 flex min-h-0 flex-1 flex-col gap-3">
                    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto [scrollbar-width:none]">
                      {statusPriority.map((status, index) => (
                        <div
                          key={status}
                          className="flex items-center gap-2 rounded-md border border-border/50 px-2 py-1.5"
                        >
                          <span className="w-4 shrink-0 text-xs text-muted-foreground">
                            {index + 1}
                          </span>
                          <span className="min-w-0 flex-1 text-sm">
                            {t(STATUS_LABEL_KEYS[status])}
                          </span>
                          <div className="flex shrink-0 gap-1">
                            <Button
                              type="button"
                              size="icon-sm"
                              variant="ghost"
                              disabled={index === 0}
                              aria-label={t("sortMoveUp")}
                              onClick={() => changeStatusPriority(index, -1)}
                            >
                              <ChevronUpIcon />
                            </Button>
                            <Button
                              type="button"
                              size="icon-sm"
                              variant="ghost"
                              disabled={index === statusPriority.length - 1}
                              aria-label={t("sortMoveDown")}
                              onClick={() => changeStatusPriority(index, 1)}
                            >
                              <ChevronDownIcon />
                            </Button>
                          </div>
                        </div>
                      ))}
                    </div>
                  </TabsContent>

                  <TabsContent
                    value="primitives"
                    className="mt-0 flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto [scrollbar-width:none]"
                  >
                    <RadioGroup
                      value={
                        pendingSort?.by === "primitives"
                          ? `${primitiveKind}:${primitiveDirection}`
                          : undefined
                      }
                      onValueChange={(value) => {
                        const [kind, direction] = value.split(":") as [
                          ServicePrimitiveKind,
                          ServiceSortDirection,
                        ]
                        selectPrimitive(kind, direction)
                      }}
                      className="flex flex-col gap-4"
                    >
                      {(
                        [
                          ["tools", "navTools"],
                          ["prompts", "prompts"],
                          ["resources", "resources"],
                        ] as const
                      ).map(([kind, labelKey]) => (
                        <FieldSet key={kind}>
                          <FieldLegend variant="label">{t(labelKey)}</FieldLegend>
                          <div className="flex flex-col gap-3">
                            <Field orientation="horizontal" className="items-center gap-2">
                              <RadioGroupItem
                                value={`${kind}:asc`}
                                id={`sort-${kind}-asc`}
                              />
                              <Label htmlFor={`sort-${kind}-asc`} className="font-normal">
                                {t("sortPrimitiveAsc")}
                              </Label>
                            </Field>
                            <Field orientation="horizontal" className="items-center gap-2">
                              <RadioGroupItem
                                value={`${kind}:desc`}
                                id={`sort-${kind}-desc`}
                              />
                              <Label htmlFor={`sort-${kind}-desc`} className="font-normal">
                                {t("sortPrimitiveDesc")}
                              </Label>
                            </Field>
                          </div>
                        </FieldSet>
                      ))}
                    </RadioGroup>
                  </TabsContent>
                </Tabs>
              </div>
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
