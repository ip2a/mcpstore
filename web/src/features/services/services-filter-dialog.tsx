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
import { Field, FieldLabel } from "@/components/ui/field"
import { Select, SelectContent, SelectGroup, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { useI18n } from "@/lib/i18n-context"

export function ServicesFilterDialog(props: {
  agentFilter: string
  agentIds: string[]
  onAgentFilterChange: (value: string) => void
}) {
  const { t } = useI18n()
  const hasActiveFilter = props.agentFilter !== "store"

  return (
    <Dialog>
      <DialogTrigger asChild>
        <Button variant="outline">
          <ListFilterIcon data-icon="inline-start" />
          {t("filter")}
          {hasActiveFilter ? (
            <Badge variant="secondary" className="h-5 min-w-5 px-1.5">
              1
            </Badge>
          ) : null}
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{t("filter")}</DialogTitle>
          <DialogDescription>{t("filterDescription")}</DialogDescription>
        </DialogHeader>
        <Field>
          <FieldLabel>{t("agent")}</FieldLabel>
          <Select value={props.agentFilter} onValueChange={props.onAgentFilterChange}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder={t("agent")} />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                <SelectItem value="store">{t("store")}</SelectItem>
                {props.agentIds.map((agentId) => (
                  <SelectItem key={agentId} value={agentId}>
                    {agentId}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
        </Field>
        <DialogFooter showCloseButton />
      </DialogContent>
    </Dialog>
  )
}
