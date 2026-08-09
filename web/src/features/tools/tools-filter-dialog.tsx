import { ListFilterIcon } from "lucide-react"

import { SearchBox } from "@/components/shared/search-box"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Field, FieldGroup, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"
import type { ServiceInstance, ToolVisibilityFilter } from "@/lib/api"

export function ToolsFilterDialog(props: {
  activeFilterCount: number
  agentId: string
  agentIds: string[]
  compact?: boolean
  instanceId: string
  onAgentIdChange: (value: string) => void
  onClearPolicy?: () => void
  onInstanceIdChange: (value: string) => void
  onQueryChange: (value: string) => void
  onScopeChange: (value: "store" | "agent") => void
  onVisibilityFilterChange: (value: ToolVisibilityFilter) => void
  query: string
  scope: "store" | "agent"
  scopeInstances: ServiceInstance[]
  visibilityFilter: ToolVisibilityFilter
}) {
  const { t } = useI18n()

  return (
    <Dialog>
      <DialogTrigger asChild>
        {props.compact ? (
          <Button variant="outline" size="icon-sm" aria-label={t("filter")} className="relative shrink-0">
            <ListFilterIcon />
            {props.activeFilterCount > 0 ? (
              <Badge variant="secondary" className="absolute -top-1 -right-1 h-4 min-w-4 px-1 text-[10px] leading-none">
                {props.activeFilterCount}
              </Badge>
            ) : null}
          </Button>
        ) : (
          <Button variant="outline" size="sm">
            <ListFilterIcon data-icon="inline-start" />
            {t("filter")}
            {props.activeFilterCount > 0 ? (
              <Badge variant="secondary" className="h-5 min-w-5 px-1.5">
                {props.activeFilterCount}
              </Badge>
            ) : null}
          </Button>
        )}
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("filter")}</DialogTitle>
          <DialogDescription>{t("toolFilterDescription")}</DialogDescription>
        </DialogHeader>
        <FieldGroup>
          {/* 搜索：独占一行 */}
          <Field>
            <FieldLabel>{t("search")}</FieldLabel>
            <SearchBox placeholder={t("searchTools")} value={props.query} onChange={props.onQueryChange} />
          </Field>
          {/* 其余筛选项：两列网格 */}
          <div className="grid grid-cols-2 gap-4">
            <Field>
              <FieldLabel>{t("availability")}</FieldLabel>
              <Select
                value={props.visibilityFilter}
                onValueChange={(value) => props.onVisibilityFilterChange(value as ToolVisibilityFilter)}
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
              <Select value={props.scope} onValueChange={(value) => props.onScopeChange(value as "store" | "agent")}>
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
                value={props.agentId || "none"}
                onValueChange={(value) => props.onAgentIdChange(value === "none" ? "" : value)}
                disabled={props.scope !== "agent"}
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
            <Field>
              <FieldLabel>{t("service")}</FieldLabel>
              <Select value={props.instanceId} onValueChange={props.onInstanceIdChange}>
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
          </div>
        </FieldGroup>
        {props.onClearPolicy ? (
          <Button variant="outline" className="w-full" onClick={props.onClearPolicy}>
            {t("clearToolPolicy")}
          </Button>
        ) : null}
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  )
}
