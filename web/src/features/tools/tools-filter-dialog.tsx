import { useState } from "react"
import { ListFilterIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
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
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
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
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolVisibilityFilter } from "@/lib/api"
import { cn } from "@/lib/utils"

export type ToolSortBy = "name" | "service"

export function ToolsFilterDialog(props: {
  activeFilterCount: number
  agentId: string
  agentIds: string[]
  hideServiceFilter?: boolean
  instanceId: string
  onAgentIdChange: (value: string) => void
  onClearPolicy?: () => void
  onInstanceIdChange: (value: string) => void
  onScopeChange: (value: "store" | "agent") => void
  onSort?: (value: ToolSortBy) => void
  onVisibilityFilterChange: (value: ToolVisibilityFilter) => void
  scope: "store" | "agent"
  scopeInstances: ServiceInstance[]
  sortBy?: ToolSortBy | null
  visibilityFilter: ToolVisibilityFilter
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [draftVisibility, setDraftVisibility] = useState(props.visibilityFilter)
  const [draftScope, setDraftScope] = useState(props.scope)
  const [draftAgentId, setDraftAgentId] = useState(props.agentId)
  const [draftInstanceId, setDraftInstanceId] = useState(props.instanceId)
  const [pendingSort, setPendingSort] = useState<ToolSortBy | null>(null)

  function beginDraft() {
    setDraftVisibility(props.visibilityFilter)
    setDraftScope(props.scope)
    setDraftAgentId(props.agentId)
    setDraftInstanceId(props.instanceId)
    setPendingSort(props.sortBy ?? null)
  }

  function onOpenChange(next: boolean) {
    if (next) beginDraft()
    setOpen(next)
  }

  function confirm() {
    props.onVisibilityFilterChange(draftVisibility)
    props.onScopeChange(draftScope)
    props.onAgentIdChange(draftAgentId)
    if (!props.hideServiceFilter) props.onInstanceIdChange(draftInstanceId)
    if (pendingSort) props.onSort?.(pendingSort)
    setOpen(false)
  }

  function resetDraft() {
    setDraftVisibility("available")
    setDraftScope("store")
    setDraftAgentId("")
    setDraftInstanceId("all")
    setPendingSort(null)
  }

  const draftFilterCount =
    (draftVisibility !== "available" ? 1 : 0) +
    (draftScope !== "store" ? 1 : 0) +
    (draftScope === "agent" && draftAgentId ? 1 : 0) +
    (!props.hideServiceFilter && draftInstanceId !== "all" ? 1 : 0)

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
          <DialogDescription>{t("toolFilterDescription")}</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[auto_minmax(0,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
          <FieldGroup className="min-h-0 overflow-y-auto [scrollbar-width:none]">
            <div className="grid grid-cols-1 gap-4 @min-[400px]:grid-cols-2">
              <Field>
                <FieldLabel>{t("availability")}</FieldLabel>
                <Select
                  value={draftVisibility}
                  onValueChange={(value) =>
                    setDraftVisibility(value as ToolVisibilityFilter)
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={t("availability")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="available">{t("available")}</SelectItem>
                      <SelectItem value="removed">{t("filterRemoved")}</SelectItem>
                      <SelectItem value="all">{t("filterAll")}</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel>{t("scope")}</FieldLabel>
                <Select
                  value={draftScope}
                  onValueChange={(value) => {
                    const next = value as "store" | "agent"
                    setDraftScope(next)
                    if (next !== "agent") setDraftAgentId("")
                  }}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={t("scope")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="store">{t("store")}</SelectItem>
                      <SelectItem value="agent">{t("agent")}</SelectItem>
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel>{t("agent")}</FieldLabel>
                <Select
                  value={draftAgentId || "none"}
                  onValueChange={(value) => setDraftAgentId(value === "none" ? "" : value)}
                  disabled={draftScope !== "agent"}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue placeholder={t("agent")} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      <SelectItem value="none">{t("noAgent")}</SelectItem>
                      {props.agentIds.map((id) => (
                        <SelectItem key={id} value={id}>
                          {id}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              {props.hideServiceFilter ? null : (
                <Field>
                  <FieldLabel>{t("service")}</FieldLabel>
                  <Select value={draftInstanceId} onValueChange={setDraftInstanceId}>
                    <SelectTrigger className="w-full">
                      <SelectValue placeholder={t("service")} />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectItem value="all">{t("allServices")}</SelectItem>
                        {props.scopeInstances.map((service) => (
                          <SelectItem key={service.instance_id} value={service.instance_id}>
                            {service.scope.type === "store"
                              ? `${service.service_name} · ${t("store")}`
                              : `${service.service_name} · ${t("agent")} ${service.scope.agent_id}`}
                          </SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
              )}
            </div>
            {props.onClearPolicy ? (
              <Button variant="outline" className="w-full" onClick={props.onClearPolicy}>
                {t("clearToolPolicy")}
              </Button>
            ) : null}
          </FieldGroup>

          <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none">
            {props.onSort ? (
              <div className="flex min-h-0 flex-1 flex-col gap-3">
                <FieldLabel className="shrink-0">{t("sortLabel")}</FieldLabel>
                <p className="text-sm text-muted-foreground">{t("sortHint")}</p>
                <RadioGroup
                  value={pendingSort ?? undefined}
                  onValueChange={(value) => setPendingSort(value as ToolSortBy)}
                  className="flex flex-col gap-3"
                >
                  <Field orientation="horizontal" className="items-center gap-2">
                    <RadioGroupItem value="name" id="tool-sort-name" />
                    <Label htmlFor="tool-sort-name" className="font-normal">
                      {t("sortName")}
                    </Label>
                  </Field>
                  <Field orientation="horizontal" className="items-center gap-2">
                    <RadioGroupItem value="service" id="tool-sort-service" />
                    <Label htmlFor="tool-sort-service" className="font-normal">
                      {t("service")}
                    </Label>
                  </Field>
                </RadioGroup>
              </div>
            ) : (
              <div className="text-sm text-muted-foreground">{t("toolFilterDescription")}</div>
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
