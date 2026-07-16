import { ListFilterIcon } from "lucide-react"

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
import {
  type ServiceScopeFilter,
  type ServiceSortBy,
  type ServiceStatusFilter,
} from "@/features/services/use-services-list"
import { useI18n } from "@/lib/i18n-context"

export function ServicesFilterDialog(props: {
  activeFilterCount: number
  agentFilter: string
  agentIds: string[]
  onAgentFilterChange: (value: string) => void
  onScopeFilterChange: (value: ServiceScopeFilter) => void
  onSortByChange: (value: ServiceSortBy) => void
  onStatusFilterChange: (value: ServiceStatusFilter) => void
  scopeFilter: ServiceScopeFilter
  sortBy: ServiceSortBy
  statusFilter: ServiceStatusFilter
}) {
  const { t } = useI18n()

  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline">
          <ListFilterIcon data-icon="inline-start" />
          {t("filter")}
          {props.activeFilterCount > 0 ? (
            <Badge variant="secondary" className="h-5 min-w-5 px-1.5">
              {props.activeFilterCount}
            </Badge>
          ) : null}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("filter")}</DialogTitle>
          <DialogDescription>{t("filterDescription")}</DialogDescription>
        </DialogHeader>
        <FieldGroup>
          <Field>
            <FieldLabel>{t("scope")}</FieldLabel>
            <Select value={props.scopeFilter} onValueChange={(value) => props.onScopeFilterChange(value as ServiceScopeFilter)}>
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
              onValueChange={(value) => props.onAgentFilterChange(value === "all" ? "" : value)}
              disabled={props.scopeFilter !== "agent"}
            >
              <SelectTrigger className="w-full">
                <SelectValue placeholder={t("agent")} />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="all">{t("filterAll")}</SelectItem>
                  {props.agentIds.map((agentId) => (
                    <SelectItem key={agentId} value={agentId}>
                      {agentId}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel>{t("status")}</FieldLabel>
            <Select
              value={props.statusFilter}
              onValueChange={(value) => props.onStatusFilterChange(value as ServiceStatusFilter)}
            >
              <SelectTrigger className="w-full">
                <SelectValue placeholder={t("status")} />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="all">{t("filterAll")}</SelectItem>
                  <SelectItem value="connected">{t("filterConnected")}</SelectItem>
                  <SelectItem value="connecting">{t("filterConnecting")}</SelectItem>
                  <SelectItem value="disconnected">{t("filterDisconnected")}</SelectItem>
                  <SelectItem value="error">{t("filterError")}</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
          <Field>
            <FieldLabel>{t("sortLabel")}</FieldLabel>
            <Select value={props.sortBy} onValueChange={(value) => props.onSortByChange(value as ServiceSortBy)}>
              <SelectTrigger className="w-full">
                <SelectValue placeholder={t("sortLabel")} />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectItem value="name">{t("sortName")}</SelectItem>
                  <SelectItem value="status">{t("sortStatus")}</SelectItem>
                  <SelectItem value="tools">{t("sortTools")}</SelectItem>
                </SelectGroup>
              </SelectContent>
            </Select>
          </Field>
        </FieldGroup>
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  )
}
