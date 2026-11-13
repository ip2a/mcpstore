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
import { cn } from "@/lib/utils"

export type FrameSessionSortBy = "time" | "frames" | "request" | "response" | "service"

export function FramesFilterDialog(props: {
  activeFilterCount: number
  onScopeFilterChange: (value: string) => void
  onSort: (value: FrameSessionSortBy) => void
  scopeFilter: string
  scopes: string[]
  sortBy: FrameSessionSortBy | null
}) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [draftScope, setDraftScope] = useState(props.scopeFilter)
  const [pendingSort, setPendingSort] = useState<FrameSessionSortBy | null>(null)

  function beginDraft() {
    setDraftScope(props.scopeFilter)
    setPendingSort(props.sortBy)
  }

  function onOpenChange(next: boolean) {
    if (next) beginDraft()
    setOpen(next)
  }

  function confirm() {
    props.onScopeFilterChange(draftScope)
    if (pendingSort) props.onSort(pendingSort)
    setOpen(false)
  }

  function resetDraft() {
    setDraftScope("all")
    setPendingSort(null)
  }

  const draftFilterCount = draftScope !== "all" ? 1 : 0

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
          <DialogDescription>{t("framesFilterDescription")}</DialogDescription>
        </DialogHeader>

        <div className="grid min-h-0 flex-1 grid-cols-1 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden px-4 py-4 @min-[640px]:grid-cols-[auto_minmax(0,1fr)] @min-[640px]:grid-rows-1 @min-[640px]:gap-5 @min-[640px]:px-5 @min-[640px]:py-4">
          <FieldGroup className="min-h-0 overflow-y-auto [scrollbar-width:none]">
            <Field>
              <FieldLabel>{t("scope")}</FieldLabel>
              <Select value={draftScope} onValueChange={setDraftScope}>
                <SelectTrigger className="w-full">
                  <SelectValue placeholder={t("scope")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectItem value="all">{t("filterAll")}</SelectItem>
                    {props.scopes.map((scope) => (
                      <SelectItem key={scope} value={scope}>
                        {scope === "store" ? t("store") : scope}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
          </FieldGroup>

          <aside className="flex h-full min-h-0 min-w-0 w-full flex-col max-h-[min(40dvh,20rem)] overflow-hidden @min-[640px]:h-full @min-[640px]:max-h-none">
            <div className="flex min-h-0 flex-1 flex-col gap-3">
              <FieldLabel className="shrink-0">{t("sortLabel")}</FieldLabel>
              <p className="text-sm text-muted-foreground">{t("sortHint")}</p>
              <RadioGroup
                value={pendingSort ?? undefined}
                onValueChange={(value) => setPendingSort(value as FrameSessionSortBy)}
                className="flex flex-col gap-3"
              >
                {(
                  [
                    ["time", "gatewayFramesColTime"],
                    ["frames", "gatewayFramesColCount"],
                    ["request", "gatewayFramesColRequest"],
                    ["response", "gatewayFramesColResponse"],
                    ["service", "service"],
                  ] as const
                ).map(([value, labelKey]) => (
                  <Field key={value} orientation="horizontal" className="items-center gap-2">
                    <RadioGroupItem value={value} id={`frame-sort-${value}`} />
                    <Label htmlFor={`frame-sort-${value}`} className="font-normal">
                      {t(labelKey)}
                    </Label>
                  </Field>
                ))}
              </RadioGroup>
            </div>
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
