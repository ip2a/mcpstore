import { ListFilterIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
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
import { SERVICE_STATUS_OPTIONS } from "@/features/services/use-services-list"
import { useI18n } from "@/lib/i18n-context"

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
  onStatusToggle: (status: ServiceDisplayStatus) => void
  scopeFilter?: ServiceScopeFilter
  statusFilter: ServiceStatusFilter
}) {
  const { t } = useI18n()

  return (
    <Dialog>
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
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("filter")}</DialogTitle>
          <DialogDescription>{t("filterDescription")}</DialogDescription>
        </DialogHeader>

        <FieldGroup>
          {props.hideScopeFilters ? null : (
            <div className="grid gap-4 sm:grid-cols-2">
              <Field>
                <FieldLabel>{t("scope")}</FieldLabel>
                <Select
                  value={props.scopeFilter}
                  onValueChange={(value) =>
                    props.onScopeFilterChange?.(value as ServiceScopeFilter)
                  }
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
                  value={props.agentFilter || "all"}
                  onValueChange={(value) =>
                    props.onAgentFilterChange?.(value === "all" ? "" : value)
                  }
                  disabled={props.scopeFilter !== "agent"}
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

          <div className="grid gap-6 sm:grid-cols-2">
            <FieldSet>
              <FieldLegend variant="label">{t("status")}</FieldLegend>
              <div data-slot="checkbox-group" className="flex flex-col gap-3">
                {SERVICE_STATUS_OPTIONS.map((status) => {
                  const id = `service-status-${status}`
                  const checked = props.statusFilter.includes(status)
                  return (
                    <Field key={status} orientation="horizontal" className="items-center gap-2">
                      <Checkbox
                        id={id}
                        checked={checked}
                        onCheckedChange={() => props.onStatusToggle(status)}
                      />
                      <Label htmlFor={id} className="font-normal">
                        {t(STATUS_LABEL_KEYS[status])}
                      </Label>
                    </Field>
                  )
                })}
              </div>
            </FieldSet>

            {props.onSort ? (
              <Field>
                <FieldLabel>{t("sortLabel")}</FieldLabel>
                <div className="flex flex-col gap-2">
                  <Button variant="outline" size="sm" onClick={() => props.onSort?.("name")}>
                    {t("sortName")}
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => props.onSort?.("status")}>
                    {t("sortStatus")}
                  </Button>
                  <Button variant="outline" size="sm" onClick={() => props.onSort?.("tools")}>
                    {t("sortTools")}
                  </Button>
                </div>
                <FieldDescription>{t("sortHint")}</FieldDescription>
              </Field>
            ) : null}
          </div>
        </FieldGroup>

        <DialogFooter showCloseButton>
          {props.activeFilterCount > 0 ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => {
                props.onScopeFilterChange?.("all")
                props.onAgentFilterChange?.("")
                props.onStatusFilterChange([])
              }}
            >
              {t("reset")}
            </Button>
          ) : null}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
